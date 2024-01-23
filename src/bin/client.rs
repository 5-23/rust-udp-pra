use std::{
    io::Write,
    net::{SocketAddr, UdpSocket},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{self, Duration, Instant, SystemTime},
};

use renet::{
    transport::{ClientAuthentication, NetcodeClientTransport, NETCODE_USER_DATA_BYTES},
    ConnectionConfig, DefaultChannel, RenetClient,
};

// Helper struct to pass an username in the user data
struct Username(String);

impl Username {
    fn to_netcode_user_data(&self) -> [u8; NETCODE_USER_DATA_BYTES] {
        let mut user_data = [0u8; NETCODE_USER_DATA_BYTES];
        if self.0.len() > NETCODE_USER_DATA_BYTES - 8 {
            // unicode(8bit)여서 8뺴줌
            panic!("Username is too big");
        }
        user_data[0..8].copy_from_slice(&(self.0.len() as u64).to_le_bytes());
        user_data[8..self.0.len() + 8].copy_from_slice(self.0.as_bytes());

        user_data
    }
}

fn main() {
    print!("ur name: ");
    std::io::stdout().flush().unwrap();
    let server_addr: SocketAddr = "127.0.0.1:4001".parse().unwrap();
    let username = Username({
        let mut s = String::new();
        std::io::stdin().read_line(&mut s).unwrap();
        s.trim().to_string()
    });
    client(server_addr, username);
}

fn client(server_addr: SocketAddr, username: Username) {
    let connection_config = ConnectionConfig::default();
    let mut client = RenetClient::new(connection_config);

    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let client_id = current_time.as_millis() as u64; // client의 고유값(시간으로 만들어짐, 시간은 겹칠수도 있으니 다른 방법을 추천)
    let authentication = ClientAuthentication::Unsecure {
        server_addr,
        client_id,
        user_data: Some(username.to_netcode_user_data()),
        protocol_id: 7, // server의 protocalid하고 같아야함
    };

    let mut transport = NetcodeClientTransport::new(current_time, authentication, socket).unwrap(); // server에 보내거나 server에서 받는 정보를 관리함
    let stdin_channel: Receiver<String> = spawn_stdin_channel();

    let mut last_update = time::Instant::now();

    loop {
        let now = time::Instant::now();
        let ping = now - last_update;
        last_update = now;
        transport
            .update(
                ping, /* 여기 아무거나 들어가도 상관 없음 */
                &mut client,
            )
            .unwrap();

        if client.is_connected() {
            match stdin_channel.try_recv() {
                Ok(text) => {
                    client.send_message(DefaultChannel::ReliableOrdered, text.as_bytes().to_vec())
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    panic!("disconnected")
                }
            }

            // 서버에서 보낸 정보를 받고 처리함
            while let Some(text) = client.receive_message(DefaultChannel::ReliableOrdered) {
                let text = String::from_utf8(text.to_vec()).unwrap();
                println!("{}", text);
            }
        }
        transport.send_packets(&mut client).unwrap();
        thread::sleep(Duration::from_micros(50)) // 이거 안쓰면 요청 많아서 client 죽음
    }
}

fn spawn_stdin_channel() -> Receiver<String> {
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || loop {
        let mut buffer = String::new();
        std::io::stdin().read_line(&mut buffer).unwrap();
        tx.send(buffer.trim_end().to_string()).unwrap(); // server에 정보를 보냄
    });
    rx
}

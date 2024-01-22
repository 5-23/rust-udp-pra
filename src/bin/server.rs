use core::time;
use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
    time::Instant,
};

use renet::{
    transport::{
        NetcodeServerTransport, ServerAuthentication, ServerConfig, NETCODE_USER_DATA_BYTES,
    },
    ClientId, ConnectionConfig, DefaultChannel, RenetServer, ServerEvent,
};

// Helper struct to pass an username in the user data
struct Username(String);

impl Username {
    fn from_user_data(user_data: &[u8; NETCODE_USER_DATA_BYTES]) -> Self {
        Self(String::from_utf8(user_data.to_vec()).unwrap())
    }
}

fn main() {
    let server_addr: SocketAddr = "127.0.0.1:4001".parse().unwrap();
    server(server_addr);
}

fn server(public_addr: SocketAddr) {
    let connection_config = ConnectionConfig::default();
    let mut server: RenetServer = RenetServer::new(connection_config); // server 초기화

    let current_time = time::Duration::from_secs(0);

    let server_config = ServerConfig {
        current_time,                                   // 그냥 시간(이거 왜넣는거지)
        max_clients: 64,                                // 연결가능한 클라이언트
        protocol_id: 7,                                 // 프로토컬 id (연결할 client하고 같아야함)
        public_addresses: vec![public_addr],            // 서버 주소
        authentication: ServerAuthentication::Unsecure, // 보안: 설정 안함
    };
    let socket: UdpSocket = UdpSocket::bind(public_addr).unwrap(); // udp소켓 열기

    let mut transport = NetcodeServerTransport::new(server_config, socket).unwrap(); // client에 보내거나 client에서 받는 정보를 관리함

    let mut usernames: HashMap<ClientId, String> = HashMap::new();
    let mut received_messages: Vec<(ClientId, String)> = vec![];
    let mut last_updated = Instant::now();

    loop {
        let now = Instant::now();
        let ping = now - last_updated;
        last_updated = now;

        // server.update(duration); // 이게 뭐하는거지
        transport
            .update(
                ping, /* 여기 아무거나 들어가도 상관 없음 */
                &mut server,
            )
            .unwrap(); // client에서 정보를 받아옴

        received_messages.clear();

        while let Some(event) = server.get_event() {
            match event {
                // connect 이벤트
                ServerEvent::ClientConnected { client_id } => {
                    let user_data = transport.user_data(client_id).unwrap(); // client 접속했을때 보내는 정보(ex| 이름, 고유ID)
                    let username = Username::from_user_data(&user_data);

                    usernames.insert(client_id, username.0); //여기에 client의 이름 저장함
                    println!("Client {} connected.", client_id); // client접속 로그

                    // client_id를 제외하고 메세지를 보냄
                    server.broadcast_message_except(
                        client_id,
                        DefaultChannel::ReliableOrdered,
                        format!(
                            "\"{}\"님이 접속했어요!",
                            String::from_utf8(user_data.to_vec()).unwrap()
                        ),
                    );
                }
                // disconnect 이벤트
                ServerEvent::ClientDisconnected { client_id, reason } => {
                    println!("Client {} disconnected: {}", client_id, reason);
                    if let Some(username) = usernames.remove(&client_id) {
                        // client_id를 제외하고 메세지를 보냄
                        server.broadcast_message_except(
                            client_id,
                            DefaultChannel::ReliableOrdered,
                            format!("\"{}\"님이 나갔어요..", username),
                        );
                    }
                }
            }
        }

        for client_id in server.clients_id() {
            while let Some(message) =
                server.receive_message(client_id, DefaultChannel::ReliableOrdered)
            // ReliableOrdered채널[기본채널]에서 client가 보낸 정보를 받아옴 (이거 왜 이벤트로 없지)
            {
                let text = String::from_utf8(message.into()).unwrap();
                let username = usernames.get(&client_id).unwrap();
                println!("Client {} ({}) sent text: {}", username, client_id, text);
                let messages = format!("{}: {}", username, text);
                received_messages.push((client_id, messages));
            }
        }

        for message in received_messages.iter() {
            // server.broadcast_message(DefaultChannel::ReliableOrdered, text.1.as_bytes().to_vec()); // 모든 유저에게 정보를 보냄

            // expect_id를 제외하고 client에 정보를 보냄
            server.broadcast_message_except(
                message.0,
                DefaultChannel::ReliableOrdered,
                message.1.clone(),
            )
        }

        // server에서 보낸다고 한 메세지를 뿌려줌
        transport.send_packets(&mut server);
    }
}

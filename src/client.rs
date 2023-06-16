use std::io::stdin;

use tonic::transport::{Channel, Endpoint};

use greet::{greeter_client::GreeterClient, HelloReq};
use voting::{voting_client::VotingClient, voting_request, VotingRequest};

pub mod voting {
    include!("../protos/voting.rs");
}

pub mod greet {
    include!("../protos/hello.rs");
}

type ThisErr = Box<dyn std::error::Error>;

async fn voting(client: &mut VotingClient<Channel>) -> Result<(), ThisErr> {
    let url = "http://helloword.com/post1";
    let mut n = 0;

    loop {
        let vote_res = if n % 2 == 0 {
            voting_request::Vote::Up
        } else {
            voting_request::Vote::Down
        };
        let request = tonic::Request::new(VotingRequest {
            url: url.to_string(),
            vote: vote_res.into(),
        });
        let response = client.vote(request).await?;
        println!("voting {}, Got: '{}'", n, response.get_ref().confirmation);
        n += 1;

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

async fn greet(client: &mut GreeterClient<Channel>) -> Result<(), ThisErr> {
    let mut n = 0;

    loop {
        let hello_content = format!("hello {}", n);
        let req = tonic::Request::new(HelloReq {
            content: hello_content,
        });
        let resp = client.say_hello(req).await?;
        println!("greet {}, Got: '{}'", n, resp.get_ref().content);

        n += 1;
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

#[tokio::main]
async fn main() -> Result<(), ThisErr> {
    // 构建一个transport::channel::Channel
    let channel = Endpoint::from_static("http://[::1]:8080").connect().await?;

    // 构建多个客户端
    let voting_client = VotingClient::new(channel.clone());
    let greet_client = GreeterClient::new(channel);

    // 负责 vote 服务
    let task_voting = tokio::spawn(async move {
        let mut c = voting_client.clone();
        if let Err(e) = voting(&mut c).await {
            println!("voting error: {}", e);
        }
    });

    // 负责 say_hello 服务
    let task_greet = tokio::spawn(async move {
        let mut c = greet_client.clone();
        if let Err(e) = greet(&mut c).await {
            println!("greet error: {}", e);
        }
    });

    tokio::try_join!(task_greet, task_voting)?;

    Ok(())
}

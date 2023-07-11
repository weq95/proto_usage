use std::{error::Error, time::Duration};

use rand::{rngs::ThreadRng, Rng};
use tokio::time;
use tonic::{
    transport::{Channel, Endpoint},
    Request,
};

use greet::{greeter_client::GreeterClient, HelloReq};
use routeguide::{route_guide_client::RouteGuideClient, Point, Rectangle, RouteNote};
use voting::{voting_client::VotingClient, voting_request, VotingRequest};

pub mod voting {
    include!("../protos/voting.rs");
}

pub mod greet {
    include!("../protos/hello.rs");
}

pub mod routeguide {
    include!("../protos/tutorial.rs");
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

async fn print_features(client: &mut RouteGuideClient<Channel>) -> Result<(), Box<dyn Error>> {
    let rectangle = Rectangle {
        lo: Some(Point {
            latitude: 400_000_000,
            longitude: -750_000_000,
        }),
        hi: Some(Point {
            latitude: 420_000_000,
            longitude: -730_000_000,
        }),
    };

    let mut stream = client
        .list_features(Request::new(rectangle))
        .await?
        .into_inner();

    while let Some(feature) = stream.message().await? {
        println!("NOTE = {:?}", feature);
    }

    Ok(())
}

async fn run_record_route(client: &mut RouteGuideClient<Channel>) -> Result<(), Box<dyn Error>> {
    let mut rng = rand::thread_rng();
    let point_count: i32 = rng.gen_range(2..100);
    let mut points = vec![];

    for _i in 0..=point_count {
        points.push(random_point());
    }

    println!("Traversing {} points", points.len());
    let request = Request::new(tokio_stream::iter(points));

    match client.record_route(request).await {
        Ok(response) => println!("SUMMARY: {:?}", response.into_inner()),
        Err(e) => println!("something went wrong: {:?}", e),
    }

    Ok(())
}

async fn run_route_chat(client: &mut RouteGuideClient<Channel>) -> Result<(), Box<dyn Error>> {
    let start = time::Instant::now();

    let outbound = async_stream::stream! {
    let mut interval = time::interval(Duration::from_secs(1));
    loop {
        let time = interval.tick().await;
        let elapsed = time.duration_since(start);
        let note = RouteNote{
          location: Some(Point{
              latitude:409146138 + elapsed.as_secs() as i32,
              longitude: -746188906,
          }),
            message: format!("at {:?}", elapsed),
        };

        yield note;
    }
    };

    let response = client.route_chat(Request::new(outbound)).await?;
    let mut inbound = response.into_inner();

    while let Some(note) = inbound.message().await? {
        println!("NOTE = {:?}", note);
    }

    Ok(())
}

fn random_point() -> Point {
    let mut rng = rand::thread_rng();
    let latitude = (rng.gen_range(0..180) - 90) * 10_000_000;
    let longitude = (rng.gen_range(0..360) - 180) * 10_000_000;

    Point {
        latitude,
        longitude,
    }
}

#[tokio::main]
async fn main() -> Result<(), ThisErr> {
    // 构建一个transport::channel::Channel
    let channel = Endpoint::from_static("http://[::1]:8080").connect().await?;

    // 构建多个客户端
    let voting_client = VotingClient::new(channel.clone());
    let greet_client = GreeterClient::new(channel.clone());
    let guide_client = RouteGuideClient::new(channel.clone());

    // 负责 vote 服务
    let _task_voting = tokio::spawn(async move {
        let mut c = voting_client.clone();
        if let Err(e) = voting(&mut c).await {
            println!("voting error: {}", e);
        }
    });

    // 负责 say_hello 服务
    let _task_greet = tokio::spawn(async move {
        let mut c = greet_client.clone();
        if let Err(e) = greet(&mut c).await {
            println!("greet error: {}", e);
        }
    });

    // tokio::try_join!(_task_greet, _task_voting);

    println!("*** SIMPLE RPC ***");
    let mut c = guide_client.clone();
    let response = c
        .get_feature(Request::new(Point {
            latitude: 409_146_138,
            longitude: -746_188_906,
        }))
        .await;
    if let Err(e) = &response {
        println!("get_feature error: {}", e);
    }
    println!("RESPONSE = {:?}", response);

    println!("\n*** SERVER STREAMING ***");
    if let Err(e) = print_features(&mut c).await {
        println!("print_features error: {}", e);
    }

    println!("\n*** CLIENT STREAMING ***");
    if let Err(e) = run_record_route(&mut c).await {
        println!("run_record_route error: {}", e);
    }

    println!("\n*** BIDIRECTIONAL STREAMING ***");
    if let Err(e) = run_route_chat(&mut c).await {
        println!("run_route_chat error: {}", e);
    }

    Ok(())
}

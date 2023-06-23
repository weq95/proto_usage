use std::pin::Pin;
use std::{cmp, collections::HashMap, sync::Arc, time::Instant};

use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, Stream, StreamExt};
use tonic::{transport::Server, Request, Response, Status, Streaming};

use greet::{
    greeter_server::{Greeter, GreeterServer},
    HelloReq, HelloResp,
};
use routeguide::{
    route_guide_server::{RouteGuide, RouteGuideServer},
    Feature, Point, Rectangle, RouteNote, RouteSummary,
};
use voting::{
    voting_server::{Voting, VotingServer},
    VotingRequest, VotingResponse,
};

pub mod voting {
    include!("../protos/voting.rs");
}

pub mod greet {
    include!("../protos/hello.rs");
}

pub mod routeguide {
    include!("../protos/tutorial.rs");
}

#[derive(Debug, Default)]
pub struct VotingService;

#[tonic::async_trait]
impl Voting for VotingService {
    async fn vote(
        &self,
        request: Request<VotingRequest>,
    ) -> Result<Response<VotingResponse>, Status> {
        let req: &VotingRequest = request.get_ref();
        match req.vote {
            0 => Ok(Response::new(VotingResponse {
                confirmation: format!("upvoted  for {}", req.url),
            })),
            1 => Ok(Response::new(VotingResponse {
                confirmation: format!("downvoted for {}", req.url),
            })),
            _ => Err(Status::new(
                tonic::Code::OutOfRange,
                "Invalid vote provided",
            )),
        }
    }
}

#[derive(Debug)]
pub struct GreetService;

#[tonic::async_trait]
impl Greeter for GreetService {
    async fn say_hello(&self, request: Request<HelloReq>) -> Result<Response<HelloResp>, Status> {
        let hello_str = request.into_inner().content;
        println!("greet from client: {}", hello_str);

        Ok(Response::new(HelloResp { content: hello_str }))
    }
}

#[derive(Debug)]
struct RouteGuideService {
    features: Arc<Vec<Feature>>,
}

#[tonic::async_trait]
impl RouteGuide for RouteGuideService {
    async fn get_feature(&self, request: Request<Point>) -> Result<Response<Feature>, Status> {
        println!("GetFeature = {:?}", request);

        for feature in &self.features[..] {
            if feature.location.as_ref() == Some(request.get_ref()) {
                return Ok(Response::new(feature.clone()));
            }
        }

        Ok(Response::new(Feature::default()))
    }

    type ListFeaturesStream = ReceiverStream<Result<Feature, Status>>;

    async fn list_features(
        &self,
        request: Request<Rectangle>,
    ) -> Result<Response<Self::ListFeaturesStream>, Status> {
        println!("ListFeatures = {:?}", request);

        let (tx, rx) = mpsc::channel(5);
        let features = self.features.clone();
        tokio::spawn(async move {
            for feature in &features[..] {
                if in_rang(feature.location.as_ref().unwrap(), request.get_ref()) {
                    println!(" => send {:?}", feature);

                    tx.send(Ok(feature.clone())).await.unwrap();
                }
            }

            println!(" /// done sending");
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn record_route(
        &self,
        request: Request<Streaming<Point>>,
    ) -> Result<Response<RouteSummary>, Status> {
        println!("RecordRoute");

        let mut stream = request.into_inner();
        let mut summary = RouteSummary::default();
        let mut last_point = None;
        let now = Instant::now();

        while let Some(point) = stream.next().await {
            let point = point?;
            println!(" ==> Point = {:?}", point);
            summary.point_count += 1;

            for feature in &self.features[..] {
                if feature.location.as_ref() == Some(&point) {
                    summary.feature_count += 1;
                }
            }

            if let Some(ref last_point) = last_point {
                summary.distance += calc_distance(last_point, &point);
            }

            last_point = Some(point);
        }

        summary.elapsed_time = now.elapsed().as_secs() as i32;

        Ok(Response::new(summary))
    }

    type RouteChatStream = Pin<Box<dyn Stream<Item = Result<RouteNote, Status>> + Send + 'static>>;

    async fn route_chat(
        &self,
        request: Request<Streaming<RouteNote>>,
    ) -> Result<Response<Self::RouteChatStream>, Status> {
        println!("RouteChat");

        let mut notes = HashMap::new();
        let mut stream = request.into_inner();

        let output = async_stream::try_stream! {
            while let Some(note) = stream.next().await {
                let note = note?;

                let location = note.location.clone().unwrap().latitude;

                let location_notes = notes.entry(location).or_insert(vec![]);
                location_notes.push(note);

                for note in location_notes {
                    yield note.clone();
                }
            }
        };

        Ok(Response::new(Box::pin(output) as Self::RouteChatStream))
    }
}

impl Eq for Point {}

fn in_rang(point: &Point, rect: &Rectangle) -> bool {
    use std::cmp;

    let lo = rect.lo.as_ref().unwrap();
    let hi = rect.hi.as_ref().unwrap();

    let left = cmp::min(lo.longitude, hi.longitude);
    let right = cmp::max(lo.longitude, hi.longitude);
    let top = cmp::max(lo.latitude, hi.latitude);
    let bottom = cmp::min(lo.latitude, hi.latitude);

    point.longitude >= left
        && point.longitude <= right
        && point.longitude >= bottom
        && point.latitude <= top
}

fn calc_distance(p1: &Point, p2: &Point) -> i32 {
    const CORD_FACTOR: f64 = 1e7;
    const R: f64 = 6_371_000.0;

    let lat1 = p1.latitude as f64 / CORD_FACTOR;
    let lat2 = p2.latitude as f64 / CORD_FACTOR;
    let lng1 = p1.longitude as f64 / CORD_FACTOR;
    let lng2 = p2.longitude as f64 / CORD_FACTOR;

    let lat_rad1 = lat1.to_radians();
    let lat_rad2 = lat2.to_radians();

    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lng = (lng2 - lng1).to_radians();

    let a = (delta_lat / 2f64).sin() * (delta_lng / 2f64).sin()
        + (lat_rad1).cos() * (lat_rad2).cos() * (delta_lng / 2f64).sin() * (delta_lng / 2f64).sin();

    let c = 2f64 * a.sqrt().atan2((1f64 - a).sqrt());

    (R * c) as i32
}

#[allow(dead_code)]
pub fn load() -> Vec<Feature> {
    /*let data_dir = std::path::PathBuf::from_iter([std::env!("CARGO_MANIFEST_DIR"), "./"]);
    let file = File::open(data_dir.join("route_guide_db.json")).expect("failed to open data file");

    let decoded: Vec<FeatureBak> =
        serde_json::from_reader(&file).expect("failed to deserialize features");

    decoded
        .into_iter()
        .map(|feature| crate::routeguide::Feature {
            name: feature.name,
            location: Some(crate::routeguide::Point {
                longitude: feature.location.longitude,
                latitude: feature.location.latitude,
            }),
        })
        .collect::<Vec<crate::routeguide::Feature>>()*/

    vec![
        crate::routeguide::Feature {
            name: "Patriots Path, Mendham, NJ 07945, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 407838351,
                longitude: -746143763,
            }),
        },
        crate::routeguide::Feature {
            name: "101 New Jersey 10, Whippany, NJ 07981, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 408122808,
                longitude: -743999179,
            }),
        },
        crate::routeguide::Feature {
            name: "U.S. 6, Shohola, PA 18458, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 413628156,
                longitude: -749015468,
            }),
        },
        crate::routeguide::Feature {
            name: "5 Conners Road, Kingston, NY 12401, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 419999544,
                longitude: -740371136,
            }),
        },
        crate::routeguide::Feature {
            name: "Mid Hudson Psychiatric Center, New Hampton, NY 10958, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 414008389,
                longitude: -743951297,
            }),
        },
        crate::routeguide::Feature {
            name: "287 Flugertown Road, Livingston Manor, NY 12758, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 419611318,
                longitude: -746524769,
            }),
        },
        crate::routeguide::Feature {
            name: "4001 Tremley Point Road, Linden, NJ 07036, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 406109563,
                longitude: -742186778,
            }),
        },
        crate::routeguide::Feature {
            name: "352 South Mountain Road, Wallkill, NY 12589, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 416802456,
                longitude: -742370183,
            }),
        },
        crate::routeguide::Feature {
            name: "Bailey Turn Road, Harriman, NY 10926, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 412950425,
                longitude: -741077389,
            }),
        },
        crate::routeguide::Feature {
            name: "193-199 Wawayanda Road, Hewitt, NJ 07421, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 412144655,
                longitude: -743949739,
            }),
        },
        crate::routeguide::Feature {
            name: "406-496 Ward Avenue, Pine Bush, NY 12566, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 415736605,
                longitude: -742847522,
            }),
        },
        crate::routeguide::Feature {
            name: "162 Merrill Road, Highland Mills, NY 10930, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 413843930,
                longitude: -740501726,
            }),
        },
        crate::routeguide::Feature {
            name: "Clinton Road, West Milford, NJ 07480, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 410873075,
                longitude: -744459023,
            }),
        },
        crate::routeguide::Feature {
            name: "16 Old Brook Lane, Warwick, NY 10990, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 412346009,
                longitude: -744026814,
            }),
        },
        crate::routeguide::Feature {
            name: "3 Drake Lane, Pennington, NJ 08534, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 402948455,
                longitude: -747903913,
            }),
        },
        crate::routeguide::Feature {
            name: "6324 8th Avenue, Brooklyn, NY 11220, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 406337092,
                longitude: -740122226,
            }),
        },
        crate::routeguide::Feature {
            name: "1 Merck Access Road, Whitehouse Station, NJ 08889, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 406421967,
                longitude: -747727624,
            }),
        },
        crate::routeguide::Feature {
            name: "78-98 Schalck Road, Narrowsburg, NY 12764, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 416318082,
                longitude: -749677716,
            }),
        },
        crate::routeguide::Feature {
            name: "282 Lakeview Drive Road, Highland Lake, NY 12743, USA".to_string(),
            location: Some(crate::routeguide::Point {
                latitude: 415301720,
                longitude: -748416257,
            }),
        },
    ]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = "[::1]:8080".parse().unwrap();
    let voting_service = VotingService::default();

    Server::builder()
        .accept_http1(true)
        .add_service(VotingServer::new(voting_service))
        .add_service(GreeterServer::new(GreetService))
        .add_service(RouteGuideServer::new(RouteGuideService {
            features: Arc::new(load()),
        }))
        .serve(address)
        .await?;

    Ok(())
}

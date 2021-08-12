use tide::{Request, Response, Body};
use tide::prelude::*;
use std::collections::HashMap;
use std::sync::{RwLock, Arc};


#[derive(Debug, Deserialize, Serialize)]
struct Dino {
    name: String,
    weight: u16,
    diet: String,
}

#[derive(Clone, Debug)]
struct State {
    dinos: Arc<RwLock<HashMap<String, Dino>>>,
}

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    tide::log::start();
    let state = State {
        dinos: Default::default(),
    };
    let mut app = tide::with_state(state);
    app.at("/").get(|_| async { Ok("Hello, world!") });

    app.at("/dinos").post(|mut req: Request<State>| async move {
        let dino: Dino = req.body_json().await?;
        let mut dinos = req.state().dinos.write().await;
        dinos.insert(String::from(&dino.name), dino);
        let mut res = Response::new(201);
        res.set_body(Body::from_json(&dino)?);

        Ok(res)
    });
    app.listen("127.0.0.1:8080").await?;
    Ok(())
}

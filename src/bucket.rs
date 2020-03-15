use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::task;

#[derive(Clone, Debug)]
pub struct Bucket {
    sender: SyncSender<i32>,
    num: Arc<Mutex<i32>>,
    pub id: Option<i32>,
}

impl Bucket {
    pub async fn request(&mut self) {
        println!("requesting access");
        let id = {
            let mut x = self.num.lock().unwrap();
            *x += 1;
            *x
        };

        let c = self.clone();
        task::spawn_blocking(move || {
            c.sender.send(id).expect("error requesting");
        })
        .await
        .expect("blocking error");
        self.id = Some(id);
    }
}

pub fn new() -> Bucket {
    println!("new bucket");

    let (sender, receiver) = sync_channel(0);

    thread::spawn(move || {
        let mut cntr = 0;
        loop {
            let _ = receiver.recv().expect("receiver error");
            cntr = cntr + 1;
        }
    });

    Bucket {
        sender,
        num: Arc::new(Mutex::new(0)),
        id: None,
    }
}

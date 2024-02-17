#![allow(clippy::similar_names)]

use std::time::Instant;

use serde::{Deserialize, Serialize};

#[cfg(all(feature = "surreal", not(feature = "mem")))]
use surrealdb::engine::local::SpeeDb as local;
#[cfg(feature = "surreal")]
use surrealdb::Surreal;

#[cfg(all(feature = "mem", feature = "surreal"))]
use surrealdb::engine::local::Mem;

#[cfg(feature = "sqlite")]
use rusqlite::{params, Connection};

#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Ord, PartialOrd)]
struct User {
    num: u64,
    first_name: String,
    last_name: String,
    age: u8,
}

impl std::fmt::Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "User: id: {}, fname: {}, lname: {}, age: {}",
            self.num, self.first_name, self.last_name, self.age
        )
    }
}

impl User {
    fn gen_random_user(age: u8, i: u64) -> Self {
        Self {
            num: i,
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            age,
        }
    }
}
const AMT: u128 = 100_000;

fn report_time(time: Instant, text: &str, amt: u128) {
    println!("{text}: {:>9}", time.elapsed().as_nanos() / amt);
}

fn insert_time(time: Instant, amt: u128) {
    report_time(time, "Insertion", amt);
}
fn select_time(time: Instant, amt: u128) {
    report_time(time, "Selection", amt);
}

#[allow(unused_variables)]
fn main() {
    let mut rng = fastrand::Rng::new();

    let mut v = Vec::new();
    for i in 1..=AMT {
        let user = User::gen_random_user(rng.u8(18..100), i as u64);
        v.push(user);
    }
    let insert_users = insert_users_hash;
    #[cfg(feature = "btree")]
    let insert_users = insert_users_btree;
    #[cfg(all(feature = "sqlite", not(feature = "surreal")))]
    let insert_users = insert_users_sql;
    #[cfg(all(feature = "surreal", not(feature = "sqlite")))]
    let insert_users = insert_users_sur;

    insert_users(v);
}

#[cfg(feature = "surreal")]
fn insert_users_sur(users: Vec<User>) {
    //let rt = tokio::runtime::Runtime::new().unwrap();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    rt.block_on(async move {
        #[cfg(feature = "mem")]
        let db = Surreal::new::<Mem>(()).await.expect("Failed to create db");
        #[cfg(not(feature = "mem"))]
        let db = Surreal::new::<local>("my.database")
            .await
            .expect("Failed to create db");

        do_it_sur(db, users).await;
    });
}

#[cfg(feature = "surreal")]
async fn do_it_sur<C>(db: Surreal<C>, users: Vec<User>)
where
    C: surrealdb::Connection + Send + Sync,
{
    db.query("define namespace test").await.unwrap();
    db.use_ns("test").await.unwrap();
    db.query("define database test").await.unwrap();
    db.use_db("test").await.unwrap();
    // db.query("define table user schemaful").await.unwrap();
    // db.query("define field first_name on table user type string")
    //     .await
    //     .unwrap();
    // db.query("define field last_name on table user type string")
    //     .await
    //     .unwrap();
    // db.query("define field age on table user type number")
    //     .await
    //     .unwrap();
    db.query("define table user schemaless").await.unwrap();

    let start = Instant::now();
    let len = users.len();
    for user in users {
        let _: Vec<User> = db.create("user").content(user).await.unwrap();
    }
    insert_time(start, len as u128);

    let start = Instant::now();
    let mut users_ret = db.query("select * from user where num > 30 and num < 50").await.unwrap();
    let mut _users_ret: Vec<User> = users_ret.take(0).unwrap();
    select_time(start, 1);
    // _users_ret.sort_unstable_by(|a, b| a.num.cmp(&b.num));
    // for user in _users_ret.into_iter() {
    //     println!("{user}");
    // }
}

#[cfg(feature = "sqlite")]
fn insert_users_sql(users: Vec<User>) {
    #[cfg(feature = "mem")]
    let conn = Connection::open_in_memory().expect("Failed to open connection");
    #[cfg(not(feature = "mem"))]
    let conn = Connection::open("my.database.sqlite").expect("Failed to open connection");

    conn.pragma_update(None, "journal_mode", "WAL").unwrap();
    conn.pragma_update(None, "synchronous", "NORMAL").unwrap();
    conn.pragma_update(None, "cache_size", "1000000").unwrap();
    conn.pragma_update(None, "temp_store", "memory").unwrap();
    if let Err(e) = conn.execute(
        "CREATE TABLE IF NOT EXISTS user (
            id INTEGER PRIMARY KEY,
            num INTEGER NOT NULL,
            first_name TEXT NOT NULL,
            last_name TEXT NOT NULL,
            age INTEGER NOT NULL
        )",
        (),
    ) {
        eprintln!("create table error: {e:?}");
    }

    let start = Instant::now();
    let len = users.len();
    for user in users {
        conn.execute(
            "INSERT INTO user (num, first_name, last_name, age) VALUES (?1, ?2, ?3, ?4)",
            params![user.num, user.first_name, user.last_name, user.age],
        )
        .expect("Failed to insert user");
    }
    insert_time(start, len as u128);

    let mut stmt = conn
        .prepare("select num, first_name, last_name, age from user where num > 30 and num < 50")
        .expect("Failed to prepare select statement");

    let start = Instant::now();
    let users_ret = stmt
        .query_map([], |row| {
            Ok(User {
                num: row.get(0)?,
                first_name: row.get(1)?,
                last_name: row.get(2)?,
                age: row.get(3)?,
            })
        })
        .expect("failed to select");

    let _users_ret: Vec<User> = users_ret.map(|user| user.unwrap()).collect();
    select_time(start, 1);
    // for user in _users_ret.into_iter() {
    //     println!("{user}");
    // }
}

fn insert_users_hash(users: Vec<User>) {
    use std::collections::HashSet;

    let len = users.len();
    let mut map: HashSet<User> = HashSet::with_capacity(len);

    let start = Instant::now();
    for user in users.into_iter() {
        map.insert(user);
    }
    insert_time(start, len as u128);

    let start = Instant::now();
    let mut _users_ret: Vec<User> = map
        .iter()
        .filter_map(|user| {
            if user.num > 30 && user.num < 50 {
                Some(user.clone())
            } else {
                None
            }
        })
        .collect();
    select_time(start, 1);
    _users_ret.sort_unstable_by(|a, b| a.num.cmp(&b.num));

    // for user in _users_ret {
    //     println!("{user}");
    // }
}

#[cfg(feature = "btree")]
fn insert_users_btree(users: Vec<User>) {
    use std::collections::BTreeSet;

    let len = users.len();
    let mut map: BTreeSet<User> = BTreeSet::new();

    let start = Instant::now();
    for user in users.into_iter() {
        map.insert(user);
    }
    insert_time(start, len as u128);

    let start = Instant::now();
    let mut _users_ret: Vec<User> = map
        .iter()
        .filter_map(|user| {
            if user.num > 30 && user.num < 50 {
                Some(user.clone())
            } else {
                None
            }
        })
        .collect();
    select_time(start, 1);
    _users_ret.sort_unstable_by(|a, b| a.num.cmp(&b.num));

    // for user in _users_ret {
    //     println!("{user}");
    // }
}

use std::{process::{Command, Stdio, ChildStdin}, io::prelude::*, sync::mpsc, path::Path, time::Duration, collections::HashSet};

use clap::Parser;
use notify::{Watcher, RecursiveMode, Config, event::EventKind};

enum Msg {
    Write(Vec<u8>),
    WriteDisplay(Vec<u8>),
}

#[derive(Parser, Debug)]
struct Args {
    #[arg(short)]
    watch_path: Option<String>,

    #[arg(short, long, default_value = "50")]
    interval: u64,
}

fn reload(path: &str, msg_tx: &mpsc::Sender<Msg>) {
    // println!("----- reload '{path}' -----");
    let msg = format!(":l {}\n", path);
    msg_tx.send(Msg::WriteDisplay(msg.into_bytes())).unwrap();

    let file = std::fs::read_to_string(path).expect("could not open watched file");
    let lines = extract_runnable_lines(&file);

    for line in lines {
        let msg = format!("{line}\n");
        msg_tx.send(Msg::WriteDisplay(msg.into_bytes())).unwrap();
    }
}

fn extract_runnable_lines(v: &str) -> Vec<&str> {
    v.lines()
        .filter_map(|v| {
            let runnable_str = "-- run:";
            if v.starts_with(runnable_str) {
                let runnable = &v[(runnable_str.len())..];

                Some(runnable)
            } else { None }
        })
    .collect()
}

fn main() {
    let args = Args::parse();

    std::thread::scope(|s| {
        let (ghc_tx, ghc_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();
        let msg_loop_tx = msg_tx.clone();

        args.watch_path.as_ref().map(|path| {
            reload(path, &msg_tx);
        });

        // read_loop
        s.spawn(move || {
            let mut stdin = std::io::stdin().lock();
            
            let mut read = [0; 128];
            loop {
                match stdin.read(&mut read) {
                    Ok(0) => { println!("<eof>"); break },
                    Ok(len) => {
                        msg_loop_tx.send(Msg::Write(read[..len].to_vec())).unwrap();
                    },
                    Err(err) => { panic!("{err}") },
                }
            }
        });


        // watch loop
        // let path = args.watch_path.clone();
        let mut watcher = notify::PollWatcher::new(move |res: Result<notify::Event, notify::Error>| {
            match res {
                Ok(ev) => {
                    let paths = ev.paths;
                    match ev.kind {
                        EventKind::Modify(_) => {
                            for path in paths {
                                path.to_str().map(|path| {
                                    reload(path, &msg_tx);
                                });
                            }
                            // reload(&path, &msg_tx);
                        },
                        _ => {

                        },
                    }
                },
                Err(err) => println!("watch error: {err}"),
            }
        }, Config::default().with_poll_interval(Duration::from_millis(args.interval))).unwrap();

        let mut watched = HashSet::new();
        args.watch_path.as_ref().map(|path| {
            watcher.watch(Path::new(path), RecursiveMode::Recursive).expect("could not find file to watch");
            watched.insert(path.clone());
        });

        // write_loop
        s.spawn(move || {
            let mut ghc_stdin: ChildStdin = ghc_rx.recv().unwrap();

            while let Ok(v) = msg_rx.recv() {
                match v {
                    Msg::Write(data) => {
                        ghc_stdin.write(&data[..]).unwrap();

                        if let Ok(str) = String::from_utf8(data) {
                            let load_str = ":l";
                            let unload_str = ":u";

                            if str.starts_with(load_str) {
                                let file = &str[load_str.len()..].trim();
                                // println!("load file: {file}");
                                if !watched.contains(*file) {
                                    watched.insert(file.to_string());
                                    watcher.watch(Path::new(file), RecursiveMode::NonRecursive).expect("could not find file to watch");
                                }
                            } else if str.starts_with(unload_str) {
                                let file = &str[unload_str.len()..].trim();
                                // println!("unload file: {file}");
                                watched.remove(*file);
                                watcher.unwatch(&Path::new(file)).expect("could not unwatch the file");
                            }
                        }
                    },
                    Msg::WriteDisplay(data) => {
                        std::thread::sleep(Duration::from_millis(100));
                        ghc_stdin.write(&data[..]).unwrap();
                        print!("{}", String::from_utf8(data).unwrap_or(format!("<invalid utf-8>")));
                    },
                }
            }
        });

        let mut cmd = Command::new("ghci")
            // .args([""])
            .stdin(Stdio::piped())
            .spawn().expect("could not find ghci");

        let stdin = cmd.stdin.take().unwrap();
        ghc_tx.send(stdin).unwrap();

        // writeln!(stdin, ":q").unwrap();

        cmd.wait().unwrap();
        std::process::exit(0);
    });
}


use std::{os::fd::{FromRawFd, OwnedFd}, process::Command};

fn main() {
    for i in 3..(1 << 24){
        let _ = unsafe{
            OwnedFd::from_raw_fd(i)
        };
    }
    let mut args = std::env::args().skip(1);
    let mut c = Command::new(args.next().unwrap());
    for arg in args {
        c.arg(arg);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = c.exec();
        panic!("cleanse: exec {:?}: {err}", c.get_program());
    }
    c.spawn()
        .unwrap_or_else(|e| panic!("cleanse: spawn {:?}: {e}", c.get_program()))
        .wait()
        .unwrap_or_else(|e| panic!("cleanse: wait {:?}: {e}", c.get_program()));
}

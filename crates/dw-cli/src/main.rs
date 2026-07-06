mod cli;
mod handlers;
mod version;

#[tokio::main]
async fn main() {
    install_broken_pipe_panic_hook();
    match std::panic::catch_unwind(|| async { handlers::run(cli::Cli::parse_localized()).await }) {
        Ok(future) => match future.await {
            Ok(()) => {}
            Err(error) if is_broken_pipe_error(&error) => {}
            Err(error) => {
                eprintln!("Error: {error}");
                std::process::exit(1);
            }
        },
        Err(payload) if is_broken_pipe_panic_payload(payload.as_ref()) => {}
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn install_broken_pipe_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if !is_broken_pipe_panic_payload(info.payload()) {
            default_hook(info);
        }
    }));
}

fn is_broken_pipe_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|error| error.kind() == std::io::ErrorKind::BrokenPipe)
    })
}

fn is_broken_pipe_panic_payload(payload: &(dyn std::any::Any + Send)) -> bool {
    panic_payload_text(payload)
        .is_some_and(|message| message.contains("Broken pipe") || message.contains("os error 32"))
}

fn panic_payload_text(payload: &(dyn std::any::Any + Send)) -> Option<&str> {
    payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broken_pipe_panic_payload_is_detected() {
        let payload = String::from("failed printing to stdout: Broken pipe (os error 32)");

        assert!(is_broken_pipe_panic_payload(&payload));
    }

    #[test]
    fn unrelated_panic_payload_is_not_broken_pipe() {
        let payload = String::from("database exploded");

        assert!(!is_broken_pipe_panic_payload(&payload));
    }

    #[test]
    fn broken_pipe_anyhow_error_is_detected() {
        let error = anyhow::Error::new(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "pipe closed",
        ));

        assert!(is_broken_pipe_error(&error));
    }
}

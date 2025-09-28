use fireworks_collaboration_lib::soak::run_from_env;

fn main() {
    match run_from_env() {
        Ok(report) => {
            println!(
                "Soak run completed. Report written to {} (duration {}s)",
                report.options.report_path, report.duration_secs
            );
        }
        Err(err) => {
            eprintln!("adaptive TLS soak failed: {err}");
            std::process::exit(1);
        }
    }
}

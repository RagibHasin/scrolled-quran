fn main() -> anyhow::Result<()> {
    scrolled_quran::run(xilem::EventLoop::with_user_event())
}

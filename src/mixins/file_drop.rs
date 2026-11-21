fn horizon_qol_file_drop(event: &winit::event::WindowEvent) -> ::core::ops::ControlFlow<()> {
    // A file dropped on the window while Horizon QoL's skin manager is open: a skin or cape to
    // import. (The game itself takes dropped files only on its title screens.)
    if let winit::event::WindowEvent::DroppedFile(path) = event
        && mod_horizon_qol::wardrobe::dropped(path)
    {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}

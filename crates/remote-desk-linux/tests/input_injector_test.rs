use remote_desk_linux::input::{
    create_linux_input_injector, InputInjectorBackend, MockInputInjector, UinputInjector,
    XTestInjector,
};
use remote_desk_proto::MouseButtonCode;

#[tokio::test]
async fn test_injector_factory_instantiation() {
    let mut injector = create_linux_input_injector(1920, 1080);
    // Should successfully accept move, click, and release
    injector.move_pointer(0.5, 0.5, false).await.expect("move");
    injector
        .mouse_button(MouseButtonCode::Left, true)
        .await
        .expect("click");
    injector.keyboard_key(30, true).await.expect("key");
    injector.release_all().await.expect("release");
}

#[tokio::test]
async fn test_failsafe_release_on_mock() {
    let mut mock = MockInputInjector::new(1920, 1080);
    mock.keyboard_key(42, true).await.unwrap(); // Shift
    mock.keyboard_key(29, true).await.unwrap(); // Ctrl
    mock.mouse_button(MouseButtonCode::Right, true)
        .await
        .unwrap();

    assert_eq!(mock.held_keys_count(), 2);
    assert_eq!(mock.held_buttons_count(), 1);

    mock.release_all().await.unwrap();

    assert_eq!(mock.held_keys_count(), 0);
    assert_eq!(mock.held_buttons_count(), 0);
}

#[tokio::test]
async fn test_xtest_or_uinput_graceful_handling() {
    let uinput_res = UinputInjector::new(1920, 1080);
    match uinput_res {
        Ok(mut u) => {
            u.release_all().await.expect("uinput release");
        }
        Err(e) => {
            println!("uinput unprivileged/absent in test sandbox: {:?}", e);
        }
    }

    let xtest_res = XTestInjector::new(1920, 1080);
    match xtest_res {
        Ok(mut x) => {
            x.release_all().await.expect("xtest release");
        }
        Err(e) => {
            println!("XTest display absent in test sandbox: {:?}", e);
        }
    }
}

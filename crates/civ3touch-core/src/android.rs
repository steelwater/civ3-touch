//! Each Java controller owns one executor thread; its engine never leaves that thread.
use crate::session::Session;
use fc3_core::{id::UnitId, types::TileCoord};
use jni::{
    objects::{JClass, JString},
    sys::{jint, jstring},
    JNIEnv,
};
use std::{
    cell::RefCell,
    panic::{catch_unwind, AssertUnwindSafe},
    path::Path,
};

thread_local! { static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) }; }

fn response(
    mut env: JNIEnv,
    action: impl FnOnce(&mut JNIEnv) -> Result<String, String>,
) -> jstring {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let json = action(&mut env)?;
        env.new_string(json)
            .map(|s| s.into_raw())
            .map_err(|e| e.to_string())
    }));
    match result {
        Ok(Ok(value)) => value,
        failure => {
            let message = match failure {
                Ok(Err(message)) => message,
                _ => {
                    SESSION.with(|s| *s.borrow_mut() = None);
                    "Rust core panicked; start a new game".into()
                }
            };
            let _ = env.throw_new("java/lang/IllegalStateException", message);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_org_civ3touch_spike_CoreBridge_newGame(
    env: JNIEnv,
    _: JClass,
    rules: JString,
) -> jstring {
    response(env, |env| {
        let path: String = env.get_string(&rules).map_err(|e| e.to_string())?.into();
        let mut game = Session::new(Path::new(&path))?;
        let value = game.initial_snapshot().to_string();
        SESSION.with(|s| *s.borrow_mut() = Some(game));
        Ok(value)
    })
}

#[no_mangle]
pub extern "system" fn Java_org_civ3touch_spike_CoreBridge_moveUnit(
    env: JNIEnv,
    _: JClass,
    index: jint,
    generation: jint,
    x: jint,
    y: jint,
) -> jstring {
    response(env, |_| {
        SESSION.with(|s| {
            let mut slot = s.borrow_mut();
            let game = slot.as_mut().ok_or("Start a new game first")?;
            Ok(game
                .move_unit(
                    UnitId {
                        index: index as u32,
                        generation: generation as u32,
                    },
                    TileCoord {
                        x: x as u32,
                        y: y as u32,
                    },
                )
                .to_string())
        })
    })
}

#[no_mangle]
pub extern "system" fn Java_org_civ3touch_spike_CoreBridge_endTurn(
    env: JNIEnv,
    _: JClass,
) -> jstring {
    response(env, |_| {
        SESSION.with(|s| {
            let mut slot = s.borrow_mut();
            Ok(slot
                .as_mut()
                .ok_or("Start a new game first")?
                .end_turn()
                .to_string())
        })
    })
}

#[no_mangle]
pub extern "system" fn Java_org_civ3touch_spike_CoreBridge_close(_: JNIEnv, _: JClass) {
    SESSION.with(|s| *s.borrow_mut() = None);
}

#[no_mangle]
pub extern "system" fn Java_org_civ3touch_spike_CoreBridge_smokeTest(
    mut env: JNIEnv,
    _: JClass,
    rules: JString,
) -> jint {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let path: String = env.get_string(&rules).map_err(|e| e.to_string())?.into();
        crate::smoke_test(Path::new(&path))
    }));
    match outcome {
        Ok(Ok(turn)) => turn as jint,
        failure => {
            let message = match failure {
                Ok(Err(message)) => message,
                _ => "Rust core panicked".into(),
            };
            let _ = env.throw_new("java/lang/IllegalStateException", message);
            -1
        }
    }
}

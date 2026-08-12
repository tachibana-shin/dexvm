//! Minimal RxJava 1 bridge. The VM is single-threaded, so streams are
//! evaluated synchronously while preserving values and terminal errors.

use super::*;
use crate::vm::object::RxOperator;

type RxPayload = (Vec<JValue>, JValue, JValue, Vec<RxOperator>);

fn rx_alloc(vm: &mut Vm, class: &str, payload: RxPayload) -> R {
    let (values, error, callable, operators) = payload;
    alloc(
        vm,
        class,
        Native::RxObservable {
            values,
            error,
            callable,
            operators,
        },
    )
}

pub(crate) fn rx_just_value(vm: &mut Vm, value: JValue) -> R {
    alloc(
        vm,
        "Lrx/Observable;",
        Native::RxObservable {
            values: vec![value],
            error: JValue::Null,
            callable: JValue::Null,
            operators: Vec::new(),
        },
    )
}

pub(crate) fn rx_error_value(vm: &mut Vm, error: JValue) -> R {
    alloc(
        vm,
        "Lrx/Observable;",
        Native::RxObservable {
            values: Vec::new(),
            error,
            callable: JValue::Null,
            operators: Vec::new(),
        },
    )
}

pub(crate) fn rx_from_result(vm: &mut Vm, result: R) -> R {
    match result {
        Ok(value) => rx_just_value(vm, value),
        Err(NatErr::Throw(error)) => rx_error_value(vm, JValue::Obj(error)),
        Err(error) => Err(error),
    }
}

fn rx_payload(vm: &mut Vm, value: JValue) -> Result<RxPayload, NatErr> {
    match payload(vm, value) {
        Some(Native::RxObservable {
            values,
            error,
            callable,
            operators,
        }) => Ok((values.clone(), *error, *callable, operators.clone())),
        _ => Err(npe(vm)),
    }
}

fn rx_materialize(vm: &mut Vm, value: JValue) -> Result<(Vec<JValue>, JValue), NatErr> {
    let (mut values, mut error, callable, operators) = rx_payload(vm, value)?;
    if !callable.is_null() {
        match inv_virt(vm, callable, "call", "()Ljava/lang/Object;", &[]) {
            Ok(value) => values.push(value),
            Err(NatErr::Throw(thrown)) => error = JValue::Obj(thrown),
            Err(other) => return Err(other),
        }
    }

    for operator in operators {
        match operator {
            RxOperator::OnErrorReturn(callback) => {
                if !error.is_null() {
                    match inv_virt(
                        vm,
                        callback,
                        "call",
                        "(Ljava/lang/Object;)Ljava/lang/Object;",
                        &[error],
                    ) {
                        Ok(value) => {
                            values = vec![value];
                            error = JValue::Null;
                        }
                        Err(NatErr::Throw(thrown)) => error = JValue::Obj(thrown),
                        Err(other) => return Err(other),
                    }
                }
            }
            RxOperator::OnErrorResumeNext(callback) => {
                if !error.is_null() {
                    match inv_virt(
                        vm,
                        callback,
                        "call",
                        "(Ljava/lang/Object;)Ljava/lang/Object;",
                        &[error],
                    ) {
                        Ok(nested) => {
                            let (nested_values, nested_error) = rx_materialize(vm, nested)?;
                            values = nested_values;
                            error = nested_error;
                        }
                        Err(NatErr::Throw(thrown)) => error = JValue::Obj(thrown),
                        Err(other) => return Err(other),
                    }
                }
            }
            RxOperator::DoOnTerminate(callback) => {
                match inv_virt(vm, callback, "run", "()V", &[]) {
                    Ok(_) => {}
                    Err(NatErr::Throw(thrown)) => error = JValue::Obj(thrown),
                    Err(other) => return Err(other),
                }
            }
            _ => {
                if !error.is_null() {
                    break;
                }
                match operator {
                    RxOperator::Map(callback) => {
                        let mut mapped = Vec::with_capacity(values.len());
                        for value in values {
                            match inv_virt(
                                vm,
                                callback,
                                "call",
                                "(Ljava/lang/Object;)Ljava/lang/Object;",
                                &[value],
                            ) {
                                Ok(value) => mapped.push(value),
                                Err(NatErr::Throw(thrown)) => {
                                    error = JValue::Obj(thrown);
                                    break;
                                }
                                Err(other) => return Err(other),
                            }
                        }
                        values = mapped;
                    }
                    RxOperator::FlatMap(callback) => {
                        let mut flattened = Vec::new();
                        for value in values {
                            let nested = match inv_virt(
                                vm,
                                callback,
                                "call",
                                "(Ljava/lang/Object;)Ljava/lang/Object;",
                                &[value],
                            ) {
                                Ok(nested) => nested,
                                Err(NatErr::Throw(thrown)) => {
                                    error = JValue::Obj(thrown);
                                    break;
                                }
                                Err(other) => return Err(other),
                            };
                            let (nested_values, nested_error) = rx_materialize(vm, nested)?;
                            if !nested_error.is_null() {
                                error = nested_error;
                                break;
                            }
                            flattened.extend(nested_values);
                        }
                        values = flattened;
                    }
                    RxOperator::DoOnNext(callback) => {
                        for value in &values {
                            match inv_virt(vm, callback, "call", "(Ljava/lang/Object;)V", &[*value])
                            {
                                Ok(_) => {}
                                Err(NatErr::Throw(thrown)) => {
                                    error = JValue::Obj(thrown);
                                    break;
                                }
                                Err(other) => return Err(other),
                            }
                        }
                    }
                    RxOperator::ToList => {
                        values = vec![list_alloc(vm, values)?];
                    }
                    RxOperator::OnErrorReturn(_)
                    | RxOperator::OnErrorResumeNext(_)
                    | RxOperator::DoOnTerminate(_) => unreachable!("handled above"),
                }
            }
        }
    }
    Ok((values, error))
}

fn rx_with_operator(vm: &mut Vm, value: JValue, operator: RxOperator) -> R {
    let (values, error, callable, mut operators) = rx_payload(vm, value)?;
    operators.push(operator);
    rx_alloc(vm, "Lrx/Observable;", (values, error, callable, operators))
}

fn observable_just(vm: &mut Vm, args: &[JValue]) -> R {
    rx_just_value(vm, args[0])
}

fn observable_error(vm: &mut Vm, args: &[JValue]) -> R {
    rx_error_value(vm, args[0])
}

fn observable_from(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    alloc(
        vm,
        "Lrx/Observable;",
        Native::RxObservable {
            values,
            error: JValue::Null,
            callable: JValue::Null,
            operators: Vec::new(),
        },
    )
}

fn observable_from_callable(vm: &mut Vm, args: &[JValue]) -> R {
    rx_alloc(
        vm,
        "Lrx/Observable;",
        (Vec::new(), JValue::Null, args[0], Vec::new()),
    )
}

fn observable_map(vm: &mut Vm, args: &[JValue]) -> R {
    rx_with_operator(vm, args[0], RxOperator::Map(args[1]))
}

fn observable_flat_map(vm: &mut Vm, args: &[JValue]) -> R {
    rx_with_operator(vm, args[0], RxOperator::FlatMap(args[1]))
}

fn observable_on_error_return(vm: &mut Vm, args: &[JValue]) -> R {
    rx_with_operator(vm, args[0], RxOperator::OnErrorReturn(args[1]))
}

fn observable_on_error_resume(vm: &mut Vm, args: &[JValue]) -> R {
    rx_with_operator(vm, args[0], RxOperator::OnErrorResumeNext(args[1]))
}

fn observable_do_on_terminate(vm: &mut Vm, args: &[JValue]) -> R {
    rx_with_operator(vm, args[0], RxOperator::DoOnTerminate(args[1]))
}

fn observable_do_on_next(vm: &mut Vm, args: &[JValue]) -> R {
    rx_with_operator(vm, args[0], RxOperator::DoOnNext(args[1]))
}

fn observable_identity(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

fn observable_to_blocking(vm: &mut Vm, args: &[JValue]) -> R {
    let payload = rx_payload(vm, args[0])?;
    rx_alloc(vm, "Lrx/observables/BlockingObservable;", payload)
}

fn blocking_first(vm: &mut Vm, args: &[JValue]) -> R {
    let (values, error) = rx_materialize(vm, args[0])?;
    if let JValue::Obj(error) = error {
        return Err(NatErr::Throw(error));
    }
    values.into_iter().next().ok_or_else(|| no_such_elem(vm))
}

fn observable_to_list(vm: &mut Vm, args: &[JValue]) -> R {
    rx_with_operator(vm, args[0], RxOperator::ToList)
}

fn schedulers_io(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lrx/Scheduler;", Native::Opaque)
}

fn subscription_unsubscribe(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

fn subscription_is_unsubscribed(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(0))
}

fn rx_terminal_err(error: JValue) -> Option<NatErr> {
    if let JValue::Obj(thrown) = error {
        Some(NatErr::Throw(thrown))
    } else {
        None
    }
}

fn observable_subscribe(vm: &mut Vm, args: &[JValue]) -> R {
    let (values, error) = rx_materialize(vm, args[0])?;
    if let Some(err) = rx_terminal_err(error) {
        inv_virt(
            vm,
            args[2],
            "call",
            "(Ljava/lang/Object;)V",
            &[JValue::Null],
        )
        .map_err(|_| err)?;
        return alloc(vm, "Lrx/Subscription;", Native::Opaque);
    }
    for value in values {
        inv_virt(vm, args[1], "call", "(Ljava/lang/Object;)V", &[value])?;
    }
    alloc(vm, "Lrx/Subscription;", Native::Opaque)
}

fn blocking_subscribe(vm: &mut Vm, args: &[JValue]) -> R {
    let (values, error) = rx_materialize(vm, args[0])?;
    if let Some(err) = rx_terminal_err(error) {
        inv_virt(
            vm,
            args[2],
            "call",
            "(Ljava/lang/Object;)V",
            &[JValue::Null],
        )
        .map_err(|_| err)?;
        return Ok(JValue::Null);
    }
    for value in values {
        inv_virt(vm, args[1], "call", "(Ljava/lang/Object;)V", &[value])?;
    }
    Ok(JValue::Null)
}

fn observable_zip2(vm: &mut Vm, args: &[JValue]) -> R {
    let (a_values, a_error) = rx_materialize(vm, args[0])?;
    let (b_values, b_error) = rx_materialize(vm, args[1])?;
    if let Some(err) = rx_terminal_err(a_error) {
        return Err(err);
    }
    if let Some(err) = rx_terminal_err(b_error) {
        return Err(err);
    }
    let len = a_values.len().min(b_values.len());
    let mut values = Vec::with_capacity(len);
    for i in 0..len {
        match inv_virt(
            vm,
            args[2],
            "call",
            "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
            &[a_values[i], b_values[i]],
        ) {
            Ok(value) => values.push(value),
            Err(NatErr::Throw(thrown)) => return rx_error_value(vm, JValue::Obj(thrown)),
            Err(other) => return Err(other),
        }
    }
    alloc(
        vm,
        "Lrx/Observable;",
        Native::RxObservable {
            values,
            error: JValue::Null,
            callable: JValue::Null,
            operators: Vec::new(),
        },
    )
}

fn observable_zip_n(vm: &mut Vm, args: &[JValue]) -> R {
    let observables = coll_elems(vm, args[0])?;
    let mut streams: Vec<Vec<JValue>> = Vec::with_capacity(observables.len());
    for observable in observables {
        let (values, error) = rx_materialize(vm, observable)?;
        if let Some(err) = rx_terminal_err(error) {
            return Err(err);
        }
        streams.push(values);
    }
    let len = streams.iter().map(Vec::len).min().unwrap_or(0);
    let mut values = Vec::with_capacity(len);
    for i in 0..len {
        let row: Vec<JValue> = streams.iter().map(|s| s[i]).collect();
        let row = alloc_arr(vm, "Ljava/lang/Object;", row.len(), move || {
            ArrayData::Obj(row)
        })?;
        match inv_virt(
            vm,
            args[1],
            "call",
            "([Ljava/lang/Object;)Ljava/lang/Object;",
            &[row],
        ) {
            Ok(value) => values.push(value),
            Err(NatErr::Throw(thrown)) => return rx_error_value(vm, JValue::Obj(thrown)),
            Err(other) => return Err(other),
        }
    }
    alloc(
        vm,
        "Lrx/Observable;",
        Native::RxObservable {
            values,
            error: JValue::Null,
            callable: JValue::Null,
            operators: Vec::new(),
        },
    )
}

fn observable_combine_latest(vm: &mut Vm, args: &[JValue]) -> R {
    let (a_values, a_error) = rx_materialize(vm, args[0])?;
    let (b_values, b_error) = rx_materialize(vm, args[1])?;
    if let Some(err) = rx_terminal_err(a_error) {
        return Err(err);
    }
    if let Some(err) = rx_terminal_err(b_error) {
        return Err(err);
    }
    let (Some(a_last), Some(b_last)) = (a_values.last().copied(), b_values.last().copied()) else {
        return rx_just_value(vm, JValue::Null);
    };
    match inv_virt(
        vm,
        args[2],
        "call",
        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
        &[a_last, b_last],
    ) {
        Ok(value) => rx_just_value(vm, value),
        Err(NatErr::Throw(thrown)) => rx_error_value(vm, JValue::Obj(thrown)),
        Err(other) => Err(other),
    }
}

fn observable_concat(vm: &mut Vm, args: &[JValue]) -> R {
    let observables = coll_elems(vm, args[0])?;
    let mut values = Vec::new();
    for observable in observables {
        let (nested_values, error) = rx_materialize(vm, observable)?;
        if let Some(err) = rx_terminal_err(error) {
            return Err(err);
        }
        values.extend(nested_values);
    }
    alloc(
        vm,
        "Lrx/Observable;",
        Native::RxObservable {
            values,
            error: JValue::Null,
            callable: JValue::Null,
            operators: Vec::new(),
        },
    )
}

fn single_from_callable(vm: &mut Vm, args: &[JValue]) -> R {
    rx_alloc(
        vm,
        "Lrx/Single;",
        (Vec::new(), JValue::Null, args[0], Vec::new()),
    )
}

fn single_identity(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

fn single_subscribe(vm: &mut Vm, args: &[JValue]) -> R {
    let (_values, error) = rx_materialize(vm, args[0])?;
    if let Some(err) = rx_terminal_err(error) {
        return Err(err);
    }
    alloc(vm, "Lrx/Subscription;", Native::Opaque)
}

pub(crate) const RX_TABLE: &[NativeEntry] = &[
    ne!(
        "Lrx/Observable;",
        "just",
        "(Ljava/lang/Object;)Lrx/Observable;",
        false,
        observable_just
    ),
    ne!(
        "Lrx/Observable;",
        "error",
        "(Ljava/lang/Throwable;)Lrx/Observable;",
        false,
        observable_error
    ),
    ne!(
        "Lrx/Observable;",
        "from",
        "(Ljava/lang/Iterable;)Lrx/Observable;",
        false,
        observable_from
    ),
    ne!(
        "Lrx/Observable;",
        "fromCallable",
        "(Ljava/util/concurrent/Callable;)Lrx/Observable;",
        false,
        observable_from_callable
    ),
    ne!(
        "Lrx/Observable;",
        "map",
        "(Lrx/functions/Func1;)Lrx/Observable;",
        true,
        observable_map
    ),
    ne!(
        "Lrx/Observable;",
        "flatMap",
        "(Lrx/functions/Func1;)Lrx/Observable;",
        true,
        observable_flat_map
    ),
    ne!(
        "Lrx/Observable;",
        "switchMap",
        "(Lrx/functions/Func1;)Lrx/Observable;",
        true,
        observable_flat_map
    ),
    ne!(
        "Lrx/Observable;",
        "doOnNext",
        "(Lrx/functions/Action1;)Lrx/Observable;",
        true,
        observable_do_on_next
    ),
    ne!(
        "Lrx/Observable;",
        "subscribeOn",
        "(Lrx/Scheduler;)Lrx/Observable;",
        true,
        observable_identity
    ),
    ne!(
        "Lrx/Observable;",
        "observeOn",
        "(Lrx/Scheduler;)Lrx/Observable;",
        true,
        observable_identity
    ),
    ne!(
        "Lrx/Observable;",
        "cache",
        "()Lrx/Observable;",
        true,
        observable_identity
    ),
    ne!(
        "Lrx/Observable;",
        "toBlocking",
        "()Lrx/observables/BlockingObservable;",
        true,
        observable_to_blocking
    ),
    ne!(
        "Lrx/Observable;",
        "toList",
        "()Lrx/Observable;",
        true,
        observable_to_list
    ),
    ne!(
        "Lrx/observables/BlockingObservable;",
        "first",
        "()Ljava/lang/Object;",
        true,
        blocking_first
    ),
    ne!(
        "Lrx/schedulers/Schedulers;",
        "io",
        "()Lrx/Scheduler;",
        false,
        schedulers_io
    ),
    ne!(
        "Lrx/Subscription;",
        "unsubscribe",
        "()V",
        true,
        subscription_unsubscribe
    ),
    ne!(
        "Lrx/Subscription;",
        "isUnsubscribed",
        "()Z",
        true,
        subscription_is_unsubscribed
    ),
    ne!(
        "Lrx/Observable;",
        "subscribe",
        "(Lrx/functions/Action1;Lrx/functions/Action1;)Lrx/Subscription;",
        true,
        observable_subscribe
    ),
    ne!(
        "Lrx/observables/BlockingObservable;",
        "subscribe",
        "(Lrx/functions/Action1;Lrx/functions/Action1;)V",
        true,
        blocking_subscribe
    ),
    ne!(
        "Lrx/Observable;",
        "zip",
        "(Lrx/Observable;Lrx/Observable;Lrx/functions/Func2;)Lrx/Observable;",
        false,
        observable_zip2
    ),
    ne!(
        "Lrx/Observable;",
        "zip",
        "(Ljava/lang/Iterable;Lrx/functions/FuncN;)Lrx/Observable;",
        false,
        observable_zip_n
    ),
    ne!(
        "Lrx/Observable;",
        "combineLatest",
        "(Lrx/Observable;Lrx/Observable;Lrx/functions/Func2;)Lrx/Observable;",
        false,
        observable_combine_latest
    ),
    ne!(
        "Lrx/Observable;",
        "concat",
        "(Ljava/lang/Iterable;)Lrx/Observable;",
        false,
        observable_concat
    ),
    ne!(
        "Lrx/Observable;",
        "onErrorReturn",
        "(Lrx/functions/Func1;)Lrx/Observable;",
        true,
        observable_on_error_return
    ),
    ne!(
        "Lrx/Observable;",
        "onErrorResumeNext",
        "(Lrx/functions/Func1;)Lrx/Observable;",
        true,
        observable_on_error_resume
    ),
    ne!(
        "Lrx/Observable;",
        "doOnTerminate",
        "(Lrx/functions/Action0;)Lrx/Observable;",
        true,
        observable_do_on_terminate
    ),
    ne!(
        "Lrx/Observable;",
        "flatMap",
        "(Lrx/functions/Func1;I)Lrx/Observable;",
        true,
        observable_flat_map
    ),
    ne!(
        "Lrx/Single;",
        "fromCallable",
        "(Ljava/util/concurrent/Callable;)Lrx/Single;",
        false,
        single_from_callable
    ),
    ne!(
        "Lrx/Single;",
        "observeOn",
        "(Lrx/Scheduler;)Lrx/Single;",
        true,
        single_identity
    ),
    ne!(
        "Lrx/Single;",
        "subscribeOn",
        "(Lrx/Scheduler;)Lrx/Single;",
        true,
        single_identity
    ),
    ne!(
        "Lrx/Single;",
        "subscribe",
        "()Lrx/Subscription;",
        true,
        single_subscribe
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Context, SandboxOptions};

    #[test]
    fn just_and_blocking_first_roundtrip() {
        let data = std::fs::read("fixtures/classes.dex").unwrap();
        let mut context = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
        let vm = context.vm();
        let observable = observable_just(vm, &[JValue::Int(42)]).unwrap();
        let blocking = observable_to_blocking(vm, &[observable]).unwrap();
        assert_eq!(blocking_first(vm, &[blocking]).unwrap(), JValue::Int(42));
    }
}

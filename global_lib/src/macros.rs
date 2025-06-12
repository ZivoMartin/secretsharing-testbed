#[macro_export]
macro_rules! dec {
    ($bytes:expr $(,$t:ty)?) => {{
        bincode::deserialize(&($bytes[..])).unwrap()
    }};
}

#[macro_export]
macro_rules! count_tts {
    () => { 0 };

    ($head:tt $($rest:tt)*) => {
        1 + count_tts!($($rest)*)
    };
}

#[macro_export]
macro_rules! log {
    ($name:ident, $msg:expr $(, $($args:expr),*)?) => {
        if global_lib::settings::VERBOSE {
            let mut f = $name.log.lock().await;
            if let Err(e) = writeln!(&*f, "{}", format!($msg $(, $($args),*)?)) {
                println!("Failed to log \"{:?}\" because of {e:?}", $msg);
            }
        }
    };
}

#[macro_export]
macro_rules! explicit_log {
    ($name:ident, $msg:expr $(, $($args:expr),*)?) => {
        println!($msg $(, $($args),*)?);
        log!($name, $msg $(, $($args),*)?);
    };
}

#[macro_export]
macro_rules! enc {
    ($var:expr) => {
        bincode::serialize(&$var).unwrap()
    };
    ($var:expr, $bytes:expr) => {
        match bincode::serialize(&$var) {
            Ok(mut serialized) => $bytes.append(&mut serialized),
            Err(e) => panic!("Serialization error: {:?}", e),
        }
    };
    ($namespace:ident, $command:expr, $val:expr $(, $extra_byte:ident)*) => {{
        let mut msg = vec![$crate::messages::NameSpace::$namespace.into(), $command.into()];
        $(msg.push($extra_byte.into());)*
        match bincode::serialize(&$val) {
            Ok(mut serialized) => msg.append(&mut serialized),
            Err(e) => panic!("Serialization error: {:?}", e),
        }
        msg
    }};
}

#[macro_export]
macro_rules! as_number {
    ($t:ty, enum $enum_name:ident { $($variant:ident),* $(,)? } $(, derive($($trait:ident),*))?) => {
        $(#[derive($($trait),*)])?
        pub enum $enum_name {
            $($variant),*
        }

        impl From<$t> for $enum_name {
            fn from(value: $t) -> Self {
                match value {
                    $(x if x == $enum_name::$variant as $t => $enum_name::$variant),*,
                    _ => panic!("Invalid value for enum: {value}"),
                }
            }
        }

        impl From<$enum_name> for $t {
            fn from(variant: $enum_name) -> Self {
                variant as $t
            }
        }
    };
}

#[macro_export]
macro_rules! select {
    (self_select, $enum_name:ident, $bytes_message:ident, $first:expr, $($variant:ident => $function:ident $($bonus_param:expr)?),* $(,)?) => {
        tokio::spawn(async move {
            match $enum_name::from($bytes_message[0]) {
                $(
                    $enum_name::$variant => $first.$function(&$bytes_message[1..], $($bonus_param)?).await,
                )*
            }
        });
    };
     (on_myself, $enum_name:ident, $bytes_message:ident, $node:ident, $($variant:ident => $function:ident $($bonus_param:expr)?),* $(,)?) => {
        tokio::spawn(async move {
            match $enum_name::from($bytes_message[0]) {
                $(
                    $enum_name::$variant => Self::$function($node, &$bytes_message[1..], $($bonus_param)?).await,
                )*
            }
        });
    };
    ($(wrapped_select,)?as_vec $enum_name:ident, $bytes_message:ident, $node:ident, $($variant:ident => $function:ident $($bonus_param:expr)?),* $(,)?) => {
        tokio::spawn(async move {
            match $enum_name::from($bytes_message.remove(0)) {
                $(
                    $enum_name::$variant => $function($node, $bytes_message, $($bonus_param)?).await,
                )*
            }
        });
    };
    ($(wrapped_select,)? $enum_name:ident, $bytes_message:ident, $node:ident, $($variant:ident => $function:ident $($bonus_param:expr)?),* $(,)?) => {
        tokio::spawn(async move {
            match $enum_name::from($bytes_message[0]) {
                $(
                    $enum_name::$variant => $function($node, &$bytes_message[1..], $($bonus_param)?).await,
                )*
            }
        });
    }
}

#[macro_export]
macro_rules! id {
    ($name:expr) => {
        $name.lock().await.op_id()
    };
}

#[macro_export]
macro_rules! wrap {
    ($name:expr) => {
        std::sync::Arc::new(tokio::sync::Mutex::new($name))
    };
}

#[macro_export]
macro_rules! wrapper_impl {
    ($struct_name:ident, $name_space:ident, $wrapped_field:ident, $(;self_meth, $($self_meth:ident $(=> $self_return_t:ty)? $(, $self_param:ident : $self_t:ty)*)*)? $(;by_name_space, $($meth:ident $(=> $return_t:ty)? $(, $param:ident : $t:ty)*)*)?) => {
        #[derive(Clone)]
        pub struct $struct_name {
            $wrapped_field: Wrapped<$name_space>,
        }

        impl Default for $struct_name {
            fn default() -> Self {
                Self {
                    $wrapped_field: $name_space::new(),
                }
            }
        }

        impl $struct_name {
            $($(
                pub async fn $self_meth(&self $(, $self_param: $self_t)*) $(-> $self_return_t)? {
                    self.$wrapped_field.lock().await.$self_meth($($self_param,)*).await
                }
            )*)?
            $($(
                pub async fn $meth(&self $(, $param: $t)*) $(-> $return_t)? {
                    $name_space::$meth(&self.$wrapped_field $(, $param)*).await
                }
            )*)?
        }
    };
}

#[macro_export]
macro_rules! as_str {
    (enum $enum_name:ident { $($variant:ident => $val:literal),* $(,)? } $(, derive($($trait:ident),*))?) => {
        $(#[derive($($trait),*)])?
        pub enum $enum_name {
            $($variant),*
        }

        impl From<&str> for $enum_name {
            fn from(s: &str) -> Self {
                match s {
                    $($val => $enum_name::$variant,)*
                    _ => panic!("Unknow string for result_type: {s}")
                }
            }
        }

        impl From<$enum_name> for &'static str {
            fn from(v: $enum_name) -> &'static str{
                match v {
                    $($enum_name::$variant => $val,)*
                }
            }
        }

        impl std::fmt::Display for $enum_name {

            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
                write!(
                    f,
                    "{}",
                    Into::<&'static str>::into(*self)
                )
            }

        }
    };
}

#[macro_export]
macro_rules! with_getters {
    (struct $struct_name:ident { $($field:ident: $t:ty),* $(,)? } $(, derive($($trait:ident),*))?) => {

        pub struct $struct_name {
            $($field: $t,)*
        }
        paste::paste! {
            impl $struct_name {
                $(
                    pub fn $field(&self) -> &$t {
                        &self.$field
                    }

                    pub fn [<$field _mut>](&mut self) -> &mut $t {
                        &mut self.$field
                    }
                )*
            }
        }
    }
}

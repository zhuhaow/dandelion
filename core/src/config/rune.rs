macro_rules! create_wrapper {
    ($name:ident, $inner:ty) => {
        #[derive(Any, Clone, Debug)]
        pub struct $name($inner);

        impl $name {
            pub fn into_inner(self) -> $inner {
                self.0
            }

            pub fn inner(&self) -> &$inner {
                &self.0
            }
        }

        impl From<$inner> for $name {
            fn from(t: $inner) -> Self {
                Self(t)
            }
        }
    };
    ($name:ident, $trait:ident, $box:ident) => {
        #[derive(Any, Debug)]
        pub struct $name($box<dyn $trait + Sync>);

        impl $name {
            pub fn into_inner(self) -> $box<dyn $trait + Sync> {
                self.0
            }

            pub fn inner(&self) -> &$box<dyn $trait + Sync> {
                &self.0
            }
        }

        impl<T: $trait + Sync + 'static> From<T> for $name {
            fn from(t: T) -> Self {
                Self($box::new(t))
            }
        }
    };
}

pub(crate) use create_wrapper;

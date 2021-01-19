use std::borrow::Cow;

pub struct ForwardRenderer<'instance, 'device> {
    pub(crate) instance: Cow<'instance, ash::Instance>,
    pub(crate) device: Cow<'device, ash::Device>,
}

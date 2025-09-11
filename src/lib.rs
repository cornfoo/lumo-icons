#![doc = r" Phosphor is a flexible icon family for interfaces, diagrams,"]
#![doc = r" presentations — whatever, really."]
#![doc = r" You can explore the available icons at [phosphoricons.com](https://phosphoricons.com)."]
#![doc = r""]
#![doc = r" ```"]
#![doc = r" use leptos::prelude::*;"]
#![doc = r" use phosphor_leptos::{Icon, IconStyle, HORSE, HEART, CUBE};"]
#![doc = r""]
#![doc = r" #[component]"]
#![doc = r" fn MyComponent() -> impl IntoView {"]
#![doc = r"     view! {"]
#![doc = r"         <Icon icon=HORSE />"]
#![doc = r##"         <Icon icon=HEART color="#AE2983" style=IconStyle::Fill size="32px" />"##]
#![doc = r#"         <Icon icon=CUBE color="teal" style=IconStyle::Duotone />"#]
#![doc = r"     }"]
#![doc = r" }"]
#![doc = r" ```"]
use leptos::{prelude::*, text_prop::TextProp};
mod icons;
pub use icons::*;
#[doc = r" An icon's style."]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconStyle {
    Core,
    Flags,
    Glass,
    MicroBold,
    SocialMedia,
    Ui,
}
#[doc = r" The SVG path data for all styles of a particular icon."]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconStyleData([&'static str; 6usize]);
impl IconStyleData {
    pub fn get(&self, style: IconStyle) -> &'static str {
        match style {
            IconStyle::Core => self.0[0usize],
            IconStyle::Flags => self.0[1usize],
            IconStyle::Glass => self.0[2usize],
            IconStyle::MicroBold => self.0[3usize],
            IconStyle::SocialMedia => self.0[4usize],
            IconStyle::Ui => self.0[5usize],
        }
    }
}
pub type IconData = &'static IconStyleData;
#[component]
pub fn Icon(
    icon: IconData,
    # [prop (into , default = Signal :: stored (IconStyle :: Core))] style: Signal<IconStyle>,
    # [prop (into , default = TextProp :: from ("1em"))] size: TextProp,
    # [prop (into , default = TextProp :: from ("currentColor"))] color: TextProp,
    # [prop (into , default = Signal :: stored (false))] mirrored: Signal<bool>,
) -> impl IntoView {
    let html = move || icon.get(style.get());
    let transform = move || mirrored.get().then_some("scale(-1, 1)");
    let height = size.clone();
    let color_attr = color.clone();
    view! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width=move || size.get()
            height=move || height.get()
            fill=move || color.get()
            color=move || color_attr.get()
            transform=transform
            viewBox=concat!("0 0 ", 256i32, " ", 256i32)
            inner_html=html
        />
    }
}

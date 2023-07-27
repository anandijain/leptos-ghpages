use leptos::*;
use leptos::{error::Result, *};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{console, window, Geolocation, Navigator, Position, PositionOptions, PositionError, Window};
use futures::channel::oneshot;
use std::sync::{Arc, Mutex};
use js_sys;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverpassResponse {
    pub elements: Vec<Element>,
    pub generator: String,
    pub osm3s: Osm3s,
    pub version: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Osm3s {
    pub copyright: String,
    #[serde(rename = "timestamp_osm_base")]
    pub timestamp_osm_base: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Element {
    pub id: i64,
    pub lat: f64,
    pub lon: f64,
    pub tags: HashMap<String, String>,
    #[serde(rename = "type")]
    pub type_field: String,
}

#[derive(Error, Clone, Debug)]
pub enum BathroomError {
    #[error("Failed to fetch bathrooms.")]
    FetchBathroomsFailed,
}

pub async fn fetch_bathrooms(_: ()) -> Result<OverpassResponse> {
    let (sender, receiver) = oneshot::channel::<Result<(f64, f64), BathroomError>>();
    let sender = Arc::new(Mutex::new(Some(sender)));

    let sender_clone = Arc::clone(&sender);
    let success_callback = Closure::wrap(Box::new(move |pos: Position| {
        let lat = pos.coords().latitude();
        let lon = pos.coords().longitude();
        log!("lat: {}, lon: {}", lat, lon);
        if let Some(sender) = sender_clone.lock().unwrap().take() {
            let _ = sender.send(Ok((lat, lon)));
        }
    }) as Box<dyn FnMut(Position)>);

    let sender_clone = Arc::clone(&sender);
    let error_callback = Closure::wrap(Box::new(move |_err: PositionError| {
        if let Some(sender) = sender_clone.lock().unwrap().take() {
            let _ = sender.send(Err(BathroomError::FetchBathroomsFailed));
        }
    }) as Box<dyn FnMut(PositionError)>);

    let navigator = window().unwrap().navigator();
    let geolocation = navigator.geolocation().unwrap();
    geolocation.get_current_position_with_error_callback(
        success_callback.as_ref().unchecked_ref(),
        Some(error_callback.as_ref().unchecked_ref()),
    ).unwrap();

    success_callback.forget();
    error_callback.forget();

    let coords = receiver.await.unwrap()?; // Propagate the BathroomError if we got one

    let (lat, lon) = coords;

    let res = reqwasm::http::Request::get(&format!(
        "https://overpass-api.de/api/interpreter?data=[out:json];node[\"amenity\"=\"toilets\"](around:2000,{lat},{lon});out;",
    ))
    .send()
    .await.unwrap()
    .json::<OverpassResponse>()
    .await.unwrap();

    Ok(res)
}

pub fn fetch_example(cx: Scope) -> impl IntoView {
    let bathrooms = create_local_resource(cx, || {}, fetch_bathrooms);
    let now = js_sys::Date::now();
    // let current_time = web_sys::window()
    // .unwrap()
    // .Date()
    // .new_0()
    // .unwrap()
    // .to_string();

    let fallback = move |cx, errors: RwSignal<Errors>| {
        let error_list = move || {
            errors.with(|errors| {
                errors
                    .iter()
                    .map(|(_, e)| view! { cx, <li>{e.to_string()}</li> })
                    .collect_view(cx)
            })
        };

        view! { cx,
            <div class="error">
                <h2>"Error"</h2>
                <ul>{error_list}</ul>
            </div>
        }
    };

    let bathrooms_view = move || {
        bathrooms.read(cx).map(|data| {
            data.map(|data| {
                let bathroom_elements = data.elements.iter().map(|element| {
                    view! { cx,
                        <tr>
                            <td>
                                <a href={format!("https://www.openstreetmap.org/node/{}", element.id)} target="_blank">OSM:{element.id}</a>
                            </td>
                            <td>
                                <a href={format!("https://www.google.com/maps/dir/?api=1&destination={},{}", element.lat, element.lon)} target="_blank">"Open in Google Maps"</a>
                            </td>
                        </tr>
                    }
                }).collect_view(cx);
    
                view! { cx,
                    <h1> {format!("Bathrooms accessed at {}", now)} </h1>
                    <table>
                    <thead>
                        <tr>
                            <th>"OSM Node"</th>
                            <th>"Directions"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {bathroom_elements}
                    </tbody>
                    </table>
                }
            })
        })
    };

    view! { cx,
        <div>
            <ErrorBoundary fallback>
                <Transition fallback=move || {
                    view! { cx, <div>"Loading (Suspense Fallback)..."</div> }
                }>
                <div>
                    {bathrooms_view}
                </div>
                </Transition>
            </ErrorBoundary>
        </div>
    }
}

pub fn main() {
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();
    mount_to_body(fetch_example)
}

use leptos::*;

use crate::model::common::DotSrcAndStyles;

#[cfg(not(feature = "server_side_graphviz"))]
use leptos::html::Div;
#[cfg(not(feature = "server_side_graphviz"))]
use leptos_meta::Script;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{prelude::wasm_bindgen, JsValue};

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(module = "/public/js/graphviz_dot_svg.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    fn graphviz_dot_svg(dot_src: String) -> Result<String, JsValue>;
}

#[cfg(feature = "server_side_graphviz")]
#[server]
pub async fn dot_svg(
    dot_src_and_styles: DotSrcAndStyles,
) -> Result<(String, String), ServerFnError> {
    use std::process::Stdio;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let DotSrcAndStyles { dot_src, styles: _ } = dot_src_and_styles;

    let mut dot_process = tokio::process::Command::new("dot")
        .arg("-Tsvg")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn dot command");

    if let Some(mut stdin) = dot_process.stdin.take() {
        stdin
            .write_all(dot_src.as_bytes())
            .await
            .map_err(|error| ServerFnError::ServerError(format!("{error}")))?;
    }

    let mut dot_svg = String::with_capacity(dot_src.len());
    if let Some(mut stdout) = dot_process.stdout.take() {
        stdout
            .read_to_string(&mut dot_svg)
            .await
            .map_err(|error| ServerFnError::ServerError(format!("{error}")))?;
    }

    let mut dot_stderr = String::new();
    if let Some(mut stderr) = dot_process.stderr.take() {
        stderr
            .read_to_string(&mut dot_stderr)
            .await
            .map_err(|error| ServerFnError::ServerError(format!("{error}")))?;
    }

    dot_process
        .wait()
        .await
        .map_err(|error| ServerFnError::ServerError(format!("{dot_stderr}{error}")))?;

    let styles = dot_svg_styles(&dot_src).await?;

    dot_svg = dot_svg
        .replacen("<g", &format!("<style>{styles}</style>\n<g"), 1)
        .replace("<g ", "<g tabindex=\"0\" ")
        .replace("fill=\"#000000\"", "")
        .replace("stroke=\"#000000\"", "")
        .replace("stroke=\"black\"", "");

    Ok((dot_svg, dot_stderr))
}

#[cfg(feature = "ssr")]
#[cfg(feature = "server_side_graphviz")]
async fn dot_svg_styles(dot_src: &str) -> Result<String, ServerFnError> {
    use std::process::Stdio;
    use tokio::io::AsyncReadExt;

    let tempdir =
        tempfile::tempdir().map_err(|error| ServerFnError::ServerError(format!("{error}")))?;

    let dot_path = tempdir.path().join("dot.dot");
    let dot_write = tokio::fs::write(&dot_path, dot_src);

    let tailwind_config_path = tempdir.path().join("tailwind.config.js");
    let tailwind_config_write = tokio::fs::write(
        &tailwind_config_path,
        b"/** @type {import('tailwindcss').Config} */\nmodule.exports = { content: ['./dot.dot'] }",
    );

    let tailwind_css_path = tempdir.path().join("tailwind.css");
    let tailwind_css_write = tokio::fs::write(
        &tailwind_css_path,
        b"\n@tailwind components;\n@tailwind utilities;\n",
    );

    let ((), (), ()) = tokio::try_join!(tailwind_config_write, dot_write, tailwind_css_write,)
        .map_err(|error| ServerFnError::ServerError(format!("{error}")))?;

    let mut tailwind_process = tokio::process::Command::new("tailwind")
        .current_dir(tempdir.path())
        .arg("-i")
        .arg(&tailwind_css_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ServerFnError::ServerError(format!("{error}")))?;

    let mut svg_styles = String::with_capacity(dot_src.len());
    if let Some(mut stdout) = tailwind_process.stdout.take() {
        stdout
            .read_to_string(&mut svg_styles)
            .await
            .map_err(|error| ServerFnError::ServerError(format!("{error}")))?;
    }

    let mut tailwind_stderr = String::new();
    if let Some(mut stderr) = tailwind_process.stderr.take() {
        stderr
            .read_to_string(&mut tailwind_stderr)
            .await
            .map_err(|error| ServerFnError::ServerError(format!("{error}")))?;
    }

    tailwind_process
        .wait()
        .await
        .map_err(|error| ServerFnError::ServerError(format!("{tailwind_stderr}{error}")))?;

    Ok(svg_styles)
}

/// Renders a graphviz graph as an SVG.
#[cfg(feature = "server_side_graphviz")]
#[component]
pub fn DotSvg<FDotSrc>(dot_src_and_styles: FDotSrc) -> impl IntoView
where
    FDotSrc: Fn() -> Option<DotSrcAndStyles> + 'static,
{
    let dot_svg_and_error_resource = create_resource(
        move || dot_src_and_styles(),
        |dot_src_and_styles| async move {
            if let Some(dot_src_and_styles) = dot_src_and_styles {
                if !dot_src_and_styles.dot_src.is_empty() {
                    match dot_svg(dot_src_and_styles).await {
                        Ok((dot_svg, error_text)) => (dot_svg, error_text),
                        Err(error) => (String::from(""), format!("{error}")),
                    }
                } else {
                    (String::from(""), String::from(""))
                }
            } else {
                (String::from(""), String::from(""))
            }
        },
    );

    view! {
        <Suspense
            fallback=move || view! { <p>"Loading..."</p> }
        >
            { move || {
                dot_svg_and_error_resource.get()
                    .map(|(dot_svg, error_text)| view! {
                        <div>
                            <div inner_html=dot_svg />

                            <div class={
                                let error_text_empty = error_text.is_empty();
                                move || {
                                    if error_text_empty {
                                        "hidden"
                                    } else {
                                        "
                                        border
                                        border-amber-300
                                        bg-gradient-to-b from-amber-100 to-amber-200
                                        rounded
                                        "
                                    }
                                }
                            }
                            >{error_text}</div>
                        </div>
                    })
            }}
        </Suspense>
    }
}

/// Renders a graphviz graph as an SVG.
///
/// TODO: Use `postcss` to generate styles on the client side via function.
///
/// * https://stackoverflow.com/questions/75787942/given-html-string-generate-the-tailwindcss-programatically
/// * https://tailwindcss.com/docs/content-configuration#configuring-raw-content
/// * https://tailwindcss.com/docs/content-configuration#safelisting-classes
///
/// ```js
/// import postcss from "postcss";
/// import tailwindcss from "tailwindcss"
/// import autoprefixer from "autoprefixer"
/// import cssnano from "cssnano"
///
/// async function generateTailwindCss(html) {
///   return (await postcss([tailwindcss({
///     content: [{ raw:html }],
///     theme: {
///       extend: {},
///     },
///     plugins: [],
///   }), autoprefixer, cssnano])
///   .process(`
///     @tailwind base;
///     @tailwind components;
///     @tailwind utilities;
///   `)).css
/// }
///
/// const css = generateTailwindCss('<div class="m-4">HTML content</div>')
/// ```
#[cfg(not(feature = "server_side_graphviz"))]
#[component]
pub fn DotSvg<FDotSrc>(dot_src_and_styles: FDotSrc) -> impl IntoView
where
    FDotSrc: Fn() -> Option<DotSrcAndStyles> + 'static,
{
    // DOM elements for the graph and error
    let svg_div_ref = create_node_ref::<Div>();

    let (error_text, set_error_text) = create_signal(None::<String>);

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _dot_src_and_styles = dot_src_and_styles;
        let _set_error_text = set_error_text;
    }

    create_effect(move |_| {
        #[cfg(not(target_arch = "wasm32"))]
        let _svg_div_ref = svg_div_ref;

        #[cfg(target_arch = "wasm32")]
        if let Some(dot_src_and_styles) = dot_src_and_styles() {
            if !dot_src_and_styles.dot_src.is_empty() {
                use std::borrow::Cow;

                let DotSrcAndStyles { dot_src, styles } = dot_src_and_styles;

                let (dot_svg, error) = match graphviz_dot_svg(dot_src) {
                    // TODO: Extract these string replacements so that they can be run from a server_function
                    //
                    // TODO: need to move tag nodes before all other nodes
                    //       so that tailwind peer selectors work.
                    Ok(dot_svg) => {
                        let dot_svg = dot_svg
                            .replacen(
                                "<g id=\"graph_0\"",
                                &format!("<styles>{styles}</styles>\n<g id=\"graph_0\""),
                                1,
                            )
                            .replace("<g ", "<g tabindex=\"0\" ")
                            .replace("fill=\"#000000\"", "")
                            .replace("stroke=\"#000000\"", "")
                            .replace("stroke=\"black\"", "");

                        (Cow::Owned(dot_svg), None)},
                    Err(error) => {
                        let error = js_sys::Error::from(error)
                            .to_string()
                            .as_string()
                            .unwrap_or_else(|| String::from("<unknown>"));

                        (Cow::Borrowed(""), Some(error))
                    }
                };

                if let Some(svg_div) = svg_div_ref.get() {
                    svg_div.set_inner_html(&dot_svg);
                }

                // ⚠️ Normally we should not write to a signal in `create_effect`, as it causes
                // state to be out of sync between server and client.
                //
                // However, for a client-side only component, we don't need to keep in sync with
                // the server.
                //
                // From Greg (creator of Leptos):
                //
                // > `create_effect` is also good for "only run this in the browser" and also for
                // > "synchronize with something non-reactive" (like a JS function) so don't worry
                // > about setting a signal inside it in that context.
                // >
                // > "Don't set a signal from an effect; rather, derive a signal." is advice meant
                // > in the sense "don't reactively read a signal inside an effect, and use it to
                // > set another signal". It's not the end of the world to do so, just not the best
                // > practice and can be hard to do correctly.
                set_error_text.update(|error_text| *error_text = error);
            }
        }
    });

    view! {
        <div>
            // Client side tailwind processing.
            <Script src="https://cdn.tailwindcss.com" />
            <div
                id="svg_div"
                node_ref=svg_div_ref
                class="overflow-auto"
            />
            <div
                id="error_div"
                class={
                    move || {
                        let error_text = error_text.get();
                        let error_text_empty = error_text
                            .as_deref()
                            .map(str::is_empty)
                            .unwrap_or(true);
                        if error_text_empty {
                            "hidden"
                        } else {
                            "
                            border
                            border-amber-300
                            bg-gradient-to-b from-amber-100 to-amber-200
                            rounded
                            "
                        }
                    }
                }
            >{
                move || {
                    let error_text = error_text.get();
                    error_text.as_deref()
                        .unwrap_or("")
                        .to_string()
                }
            }</div>
        </div>
    }
}

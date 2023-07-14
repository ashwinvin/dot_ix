use std::{
    borrow::Cow,
    fmt::{self, Write},
};

use indexmap::IndexMap;

use crate::{
    model::{
        common::{EdgeId, GraphvizDotTheme, NodeHierarchy, NodeId},
        info_graph::{InfoGraph, NodeInfo},
    },
    rt::IntoGraphvizDotSrc,
};

/// Renders a GraphViz Dot diagram with interactive styling.
///
/// This is currently a mashed together implementation. A proper implementation
/// would require investigation into:
///
/// * Whether colours and border sizes should be part of a theme object that we
///   take in.
/// * Whether colours and border sizes can be configured through CSS classes,
///   and within the generated dot source, we only apply those classes.
impl IntoGraphvizDotSrc for &InfoGraph {
    fn into(self, theme: &GraphvizDotTheme) -> String {
        let graph_attrs = graph_attrs(theme);
        let node_attrs = node_attrs(theme);
        let edge_attrs = edge_attrs(theme);

        let node_clusters = self
            .hierarchy()
            .iter()
            .map(|(node_id, node_hierarchy)| {
                node_cluster(theme, self.node_infos(), node_id, node_hierarchy)
            })
            .collect::<Vec<String>>()
            .join("\n");

        let edges = self
            .edges()
            .iter()
            .map(|(edge_id, [src_node_id, target_node_id])| {
                edge(self.hierarchy(), edge_id, src_node_id, target_node_id)
            })
            .collect::<Vec<String>>()
            .join("\n");

        format!(
            "digraph G {{
                {graph_attrs}
                {node_attrs}
                {edge_attrs}

                {node_clusters}

                {edges}
            }}"
        )
    }
}

fn graph_attrs(theme: &GraphvizDotTheme) -> String {
    let plain_text_color = theme.plain_text_color();
    // Note: `margin` is set to 0.1 because some text lies outside the viewport.
    // This may be due to incorrect width calculation for emoji characters, which
    // GraphViz falls back to the space character width.
    format!(
        "\
            compound  = true\n\
            graph [\n\
                margin    = 0.1\n\
                penwidth  = 0\n\
                nodesep   = 0.0\n\
                ranksep   = 0.02\n\
                bgcolor   = \"transparent\"\n\
                fontname  = \"helvetica\"\n\
                fontcolor = \"{plain_text_color}\"\n\
                splines   = line\n\
                rankdir   = LR\n\
            ]\n\
        "
    )
}

fn node_attrs(theme: &GraphvizDotTheme) -> String {
    let node_text_color = theme.node_text_color();
    format!(
        "\
            node [\n\
                fontcolor = \"{node_text_color}\"\n\
                fontname  = \"monospace\"\n\
                fontsize  = 12\n\
                shape     = \"rect\"\n\
                style     = \"rounded,filled\"\n\
                width     = 0.3\n\
                height    = 0.3\n\
                margin    = 0.04\n\
                color     = \"#9999aa\"\n\
                fillcolor = \"#ddddf5\"\n\
            ]\n\
        "
    )
}

fn edge_attrs(theme: &GraphvizDotTheme) -> String {
    let edge_color = theme.edge_color();
    let plain_text_color = theme.plain_text_color();
    format!(
        "\
            edge [\n\
                arrowsize = 0.7\n\
                color     = \"{edge_color}\"\n\
                fontcolor = \"{plain_text_color}\"\n\
            ]\n\
        "
    )
}

fn node_cluster(
    theme: &GraphvizDotTheme,
    node_infos: &IndexMap<NodeId, NodeInfo>,
    node_id: &NodeId,
    node_hierarchy: &NodeHierarchy,
) -> String {
    let mut buffer = String::with_capacity(1024);

    node_cluster_internal(theme, node_infos, node_id, node_hierarchy, &mut buffer)
        .expect("Failed to write node_cluster string.");

    buffer
}

fn node_cluster_internal(
    theme: &GraphvizDotTheme,
    node_infos: &IndexMap<NodeId, NodeInfo>,
    node_id: &NodeId,
    node_hierarchy: &NodeHierarchy,
    buffer: &mut String,
) -> fmt::Result {
    let node_info = node_infos.get(node_id);
    let node_label = node_info.map(NodeInfo::name).unwrap_or(&node_id);
    let classes = "\
            [&>path]:fill-slate-300 \
            [&>path]:stroke-1 \
            [&>path]:stroke-slate-600 \
            [&>path]:hover:fill-slate-200 \
            [&>path]:hover:stroke-slate-600 \
            [&>path]:hover:stroke-2 \
            cursor-pointer \
        ";

    if node_hierarchy.is_empty() {
        write!(
            buffer,
            r#"
                {node_id} [
                    label = <<table
                        border="0"
                        cellborder="0"
                        cellpadding="0">
                        <tr>
                            <td><font point-size="15">📥</font></td>
                            <td balign="left">{node_label}</td>
                        </tr>
                    </table>>
                    class = "{classes}"
                ]
            "#
        )?;
    } else {
        write!(
            buffer,
            r#"
                subgraph cluster_{node_id} {{
                    label = <<table
                        border="0"
                        cellborder="0"
                        cellpadding="0">
                        <tr>
                            <td><font point-size="15">📥</font></td>
                            <td balign="left">{node_label}</td>
                        </tr>
                    </table>>
                    style = "filled,rounded"
                    class = "{classes}"
            "#
        )?;

        node_hierarchy
            .iter()
            .try_for_each(|(child_node_id, child_node_hierarchy)| {
                node_cluster_internal(
                    theme,
                    node_infos,
                    child_node_id,
                    child_node_hierarchy,
                    buffer,
                )
            })?;

        write!(buffer, "}}")?;
    }

    Ok(())
}

fn edge(
    node_hierarchy: &NodeHierarchy,
    edge_id: &EdgeId,
    src_node_id: &NodeId,
    target_node_id: &NodeId,
) -> String {
    let (edge_src_node_id, ltail) = if let Some(node_hierarchy) = node_hierarchy.get(src_node_id) {
        let edge_src_node_id = node_hierarchy
            .last()
            .map(|(last_child_node_id, _)| last_child_node_id)
            .unwrap_or(src_node_id);
        let ltail = Cow::Owned(format!(", ltail = cluster_{src_node_id}"));

        (edge_src_node_id, ltail)
    } else {
        (src_node_id, Cow::Borrowed(""))
    };

    let (edge_target_node_id, lhead) =
        if let Some(node_hierarchy) = node_hierarchy.get(target_node_id) {
            let edge_target_node_id = node_hierarchy
                .first()
                .map(|(first_child_node_id, _)| first_child_node_id)
                .unwrap_or(target_node_id);
            let lhead = Cow::Owned(format!(", lhead = cluster_{target_node_id}"));

            (edge_target_node_id, lhead)
        } else {
            (target_node_id, Cow::Borrowed(""))
        };

    format!(
        r#"{edge_src_node_id} -> {edge_target_node_id} [id = "{edge_id}", minlen = 9 {ltail} {lhead}]"#
    )
}
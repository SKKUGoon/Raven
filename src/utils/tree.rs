use crate::config::Settings;
use crate::proto::control_client::ControlClient;
use crate::proto::ListRequest;
use crate::utils::service_registry;
use ptree::TreeBuilder;

pub async fn show_users_tree(settings: &Settings) {
    let services = service_registry::all_services(settings);

    let mut tree = TreeBuilder::new("Raven Cluster".to_string());

    for svc in services {
        let addr = svc.addr(&settings.server.host);
        let status = match ControlClient::connect(addr).await {
            Ok(mut client) => match client.list_collections(ListRequest {}).await {
                Ok(resp) => {
                    let collections = resp.into_inner().collections;
                    let node_text = format!("{} ({} active)", svc.display_name, collections.len());
                    let service_node = tree.begin_child(node_text);

                    for c in collections {
                        let info = format!("{} [subs: {}]", c.symbol, c.subscriber_count);
                        service_node.add_empty_child(info);
                    }
                    service_node.end_child();
                    true
                }
                Err(_) => false,
            },
            Err(_) => false,
        };

        if !status {
            // Service is down, maybe show it?
            let node_text = format!("{} (UNREACHABLE)", svc.display_name);
            tree.add_empty_child(node_text);
        }
    }

    let _ = ptree::print_tree(&tree.build());
}


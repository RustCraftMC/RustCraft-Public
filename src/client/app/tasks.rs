use crate::client;
use crate::net;
use std::sync::mpsc::{self, Receiver};

pub(super) struct AuthTask {
    rx: Receiver<Result<crate::auth::models::Account, String>>,
}

impl AuthTask {
    pub(super) fn login() -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = crate::auth::service::AuthService::full_login().map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        Self { rx }
    }

    pub(super) fn try_finish(&self) -> Option<Result<crate::auth::models::Account, String>> {
        self.rx.try_recv().ok()
    }
}

pub(super) struct ServerRefreshTask {
    rx: Receiver<client::server_list::ServerList>,
}

impl ServerRefreshTask {
    pub(super) fn spawn(mut servers: client::server_list::ServerList) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            servers.refresh_statuses();
            let _ = tx.send(servers);
        });
        Self { rx }
    }

    pub(super) fn try_finish(&self) -> Option<client::server_list::ServerList> {
        self.rx.try_recv().ok()
    }
}

pub(super) struct ConnectTask {
    rx: Receiver<ConnectResult>,
    pub(super) address: String,
}

pub(super) type ConnectResult = Result<net::connection::Connection, String>;

impl ConnectTask {
    pub(super) fn spawn(
        address: String,
        username: String,
        account: Option<crate::auth::models::Account>,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let thread_address = address.clone();
        std::thread::spawn(move || {
            let refreshed = if account.is_some() {
                crate::auth::service::AuthService::active_account()
                    .ok()
                    .or(account)
            } else {
                None
            };
            let login_name = refreshed
                .as_ref()
                .and_then(|account| account.username.as_deref())
                .unwrap_or(&username);
            let result = net::connection::Connection::connect(
                &thread_address,
                login_name,
                refreshed.as_ref(),
            )
            .map_err(|err| err.to_string());
            let _ = tx.send(result);
        });
        Self { rx, address }
    }

    pub(super) fn try_finish(&self) -> Option<ConnectResult> {
        self.rx.try_recv().ok()
    }
}

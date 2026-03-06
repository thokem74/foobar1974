use zbus::Connection;

pub async fn start_mpris() -> anyhow::Result<Connection> {
    let conn = Connection::session().await?;
    Ok(conn)
}

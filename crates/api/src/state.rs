use clickhouse::Client;
use mongodb::Database;
use purestat_config::Settings;
use purestat_services::analytics::ingest::IngestService;
use purestat_services::analytics::privacy::PrivacyEngine;
use purestat_services::analytics::query::QueryService;
use purestat_services::analytics::realtime::RealtimeService;
use purestat_services::export::ExportService;
use purestat_services::stripe::StripeService;
use purestat_services::{
    ApiKeyDao, AuthService, GoalDao, InviteDao, OrgDao, OrgMemberDao, SiteDao, UserDao,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub settings: Settings,
    pub auth: Arc<AuthService>,
    pub users: Arc<UserDao>,
    pub orgs: Arc<OrgDao>,
    pub org_members: Arc<OrgMemberDao>,
    pub sites: Arc<SiteDao>,
    pub goals: Arc<GoalDao>,
    pub invites: Arc<InviteDao>,
    pub api_keys: Arc<ApiKeyDao>,
    pub ingest: Arc<IngestService>,
    pub query: Arc<QueryService>,
    pub realtime: Arc<RealtimeService>,
    pub export: Arc<ExportService>,
    pub stripe: Arc<StripeService>,
    pub privacy: Arc<PrivacyEngine>,
}

impl AppState {
    pub async fn new(
        db: Database,
        ch: Client,
        redis: redis::aio::ConnectionManager,
        settings: Settings,
    ) -> anyhow::Result<Self> {
        let auth = Arc::new(AuthService::new(settings.jwt.clone()));
        let users = Arc::new(UserDao::new(&db));
        let orgs = Arc::new(OrgDao::new(&db));
        let org_members = Arc::new(OrgMemberDao::new(&db));
        let sites = Arc::new(SiteDao::new(&db));
        let goals = Arc::new(GoalDao::new(&db));
        let invites = Arc::new(InviteDao::new(&db));
        let api_keys = Arc::new(ApiKeyDao::new(&db));

        let ingest = Arc::new(IngestService::new(
            ch.clone(),
            settings.tracker.batch_size,
        ));
        ingest.start_flush_timer(settings.tracker.flush_interval_ms);

        let query = Arc::new(QueryService::new(ch.clone()));
        let realtime = Arc::new(RealtimeService::new(ch.clone()));
        let export = Arc::new(ExportService::new(ch));
        let stripe = Arc::new(StripeService::new(settings.stripe.clone()));
        let privacy = Arc::new(PrivacyEngine::new(
            redis,
            settings.privacy.salt_ttl_hours,
        ));

        Ok(Self {
            db,
            settings,
            auth,
            users,
            orgs,
            org_members,
            sites,
            goals,
            invites,
            api_keys,
            ingest,
            query,
            realtime,
            export,
            stripe,
            privacy,
        })
    }
}

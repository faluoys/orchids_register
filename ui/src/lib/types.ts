export interface Account {
  id: number;
  email: string;
  password: string;
  sign_up_id: string | null;
  email_code: string | null;
  register_complete: boolean;
  created_session_id: string | null;
  created_user_id: string | null;
  client_cookie: string | null;
  client_uat: string | null;
  desktop_jwt: string | null;
  status: "pending" | "running" | "complete" | "failed";
  error_message: string | null;
  batch_id: string | null;
  plan: string | null;
  credits: number | null;
  created_at: string;
  updated_at: string;
  group_id: number;
  group_name: string;
}

export interface AccountGroup {
  id: number;
  name: string;
  pinned: boolean;
  is_default: boolean;
  sort_order: number;
  created_at: string;
  updated_at: string;
}

export interface Domain {
  id: number;
  domain: string;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface LogEntry {
  step: string;
  message: string;
  level: string;
  timestamp: string;
}

export interface BatchProgress {
  completed: number;
  failed: number;
  total: number;
  current_email: string | null;
}

export interface BatchComplete {
  batch_id: string;
  completed: number;
  failed: number;
  total: number;
}

export interface RegisterArgs {
  email: string | null;
  password: string | null;
  captcha_token: string | null;
  use_capmonster: boolean;
  captcha_api_url: string;
  captcha_timeout: number;
  captcha_poll_interval: number;
  captcha_website_url: string;
  captcha_website_key: string;
  email_code: string | null;
  locale: string;
  timeout: number;
  mail_mode: string;
  mail_gateway_base_url: string | null;
  mail_gateway_api_key: string | null;
  mail_provider: string;
  mail_provider_mode: string;
  mail_project_code: string | null;
  mail_domain: string | null;
  poll_timeout: number;
  poll_interval: number;
  code_pattern: string;
  debug_email: boolean;
  test_desktop_session: boolean;
  proxy: string | null;
  use_proxy_pool: boolean;
  proxy_pool_api: string;
}

export type ManagedServiceName = "mail_gateway" | "turnstile_solver";

export type ServiceSource = "stopped" | "desktop_managed" | "external";

export interface ServiceStatus {
  running: boolean;
  pid: number | null;
  last_started_at: string | null;
  last_error: string | null;
  source: ServiceSource;
}

export interface MailGatewayHealthResult {
  status: string;
  timestamp: number;
  providers: Record<string, string>;
}

export type Page = "dashboard" | "register" | "accounts" | "groups" | "domains" | "inbox_config" | "settings";

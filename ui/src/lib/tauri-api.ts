import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  Account,
  AccountGroup,
  Domain,
  MailGatewayHealthResult,
  ManagedServiceName,
  RegisterArgs,
  ServiceStatus,
} from "./types";

export interface ServiceStatusChangedEvent {
  service: ManagedServiceName;
  status: ServiceStatus;
}

// Register
export async function startRegistration(args: RegisterArgs): Promise<string> {
  return invoke("start_registration", { args });
}

export async function startBatchRegistration(
  args: RegisterArgs,
  count: number,
  concurrency: number
): Promise<string> {
  return invoke("start_batch_registration", { args, count, concurrency });
}

export async function cancelBatch(): Promise<void> {
  return invoke("cancel_batch");
}

// Accounts
export async function getAccounts(
  status?: string,
  groupId?: number
): Promise<Account[]> {
  return invoke("get_accounts", { status: status ?? null, groupId: groupId ?? null });
}

export async function refreshAccountsProfileMissing(limit = 20): Promise<number> {
  return invoke("refresh_accounts_profile_missing", { limit });
}

export async function refreshAccountProfile(id: number): Promise<Account> {
  return invoke("refresh_account_profile", { id });
}

export async function deleteAccount(id: number): Promise<void> {
  return invoke("delete_account", { id });
}

export async function deleteAccounts(ids: number[]): Promise<number> {
  return invoke("delete_accounts", { ids });
}

export async function exportAccounts(
  status?: string,
  format?: string,
  ids?: number[]
): Promise<string> {
  return invoke("export_accounts", {
    status: status ?? null,
    format: format ?? null,
    ids: ids ?? null,
  });
}

export async function listAccountGroups(): Promise<AccountGroup[]> {
  return invoke("list_account_groups");
}

export async function createAccountGroup(name: string): Promise<AccountGroup> {
  return invoke("create_account_group", { name });
}

export async function renameAccountGroup(id: number, name: string): Promise<void> {
  return invoke("rename_account_group", { id, name });
}

export async function deleteAccountGroup(id: number): Promise<void> {
  return invoke("delete_account_group", { id });
}

export async function setAccountGroupPinned(id: number, pinned: boolean): Promise<void> {
  return invoke("set_account_group_pinned", { id, pinned });
}

export async function moveAccountGroup(id: number, direction: "up" | "down"): Promise<void> {
  return invoke("move_account_group", { id, direction });
}

export async function moveAccountsToGroup(ids: number[], targetGroupId: number): Promise<number> {
  return invoke("move_accounts_to_group", { ids, targetGroupId });
}

// Domains
export async function listDomains(): Promise<Domain[]> {
  return invoke("list_domains");
}

export async function createDomain(domain: string, enabled = true): Promise<Domain> {
  return invoke("create_domain", { domain, enabled });
}

export async function updateDomain(id: number, domain: string, enabled: boolean): Promise<void> {
  return invoke("update_domain", { id, domain, enabled });
}

export async function deleteDomain(id: number): Promise<void> {
  return invoke("delete_domain", { id });
}

export async function saveTextFile(content: string, defaultName: string): Promise<boolean> {
  return invoke("save_text_file", { content, defaultName });
}

// Config
export async function getAllConfig(): Promise<Record<string, string>> {
  return invoke("get_all_config");
}

export async function saveConfig(
  configs: Record<string, string>
): Promise<void> {
  return invoke("save_config", { configs });
}

export async function resetConfig(): Promise<void> {
  return invoke("reset_config");
}

export async function testProxy(): Promise<{ ip: string; country: string; city: string }> {
  return invoke("test_proxy");
}

export async function testMailGatewayHealth(
  baseUrl: string,
  apiKey: string | null
): Promise<MailGatewayHealthResult> {
  return invoke("test_mail_gateway_health", { baseUrl, apiKey });
}

export async function getServiceStatus(): Promise<Record<ManagedServiceName, ServiceStatus>> {
  return invoke("get_service_status");
}

export async function startMailGateway(): Promise<ServiceStatus> {
  return invoke("start_mail_gateway");
}

export async function stopMailGateway(): Promise<ServiceStatus> {
  return invoke("stop_mail_gateway");
}

export async function startTurnstileSolver(): Promise<ServiceStatus> {
  return invoke("start_turnstile_solver");
}

export async function stopTurnstileSolver(): Promise<ServiceStatus> {
  return invoke("stop_turnstile_solver");
}

export async function cancelClosePrompt(): Promise<void> {
  return invoke("cancel_close_prompt");
}

export async function confirmExit(): Promise<void> {
  return invoke("confirm_exit");
}

export async function onServiceStatusChanged(
  handler: (event: ServiceStatusChangedEvent) => void | Promise<void>
): Promise<() => void> {
  return listen<ServiceStatusChangedEvent>("service-status-updated", async (event) => {
    await handler(event.payload);
  });
}

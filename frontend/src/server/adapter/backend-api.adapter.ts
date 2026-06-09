/**
 * Rust backend API adapter — single source of truth for all HTTP calls.
 *
 * Route structure:
 *   Public (no auth prefix):  /auth/login  /auth/register  /auth/forgot-password  /auth/reset-password
 *   Authenticated (prefixed): /api/v1/<everything else>
 *
 * Pass the JWT returned by login as `token` — it is forwarded as
 *   Authorization: Bearer <token>
 *
 * All field names match the Rust structs exactly (snake_case).
 */

function backendBase(value: string | undefined): string {
    return (value?.trim() || 'http://localhost:3001').replace(/\/+$/, '');
}

const BASE =
    typeof window === 'undefined'
        ? backendBase(process.env.BACKEND_URL)
        : backendBase(process.env.NEXT_PUBLIC_BACKEND_URL);

const V1 = `${BASE}/api/v1`;

// ── Error type ────────────────────────────────────────────────────────────────

export class BackendApiError extends Error {
    constructor(public readonly status: number, message: string) {
        super(message);
        this.name = 'BackendApiError';
    }
}

// ── Core fetch ────────────────────────────────────────────────────────────────

async function req<T>(
    url: string,
    options: RequestInit & { token?: string } = {},
): Promise<T> {
    const { token, headers: extraHeaders, ...init } = options;
    const headers: Record<string, string> = {
        'Content-Type': 'application/json',
        ...(extraHeaders as Record<string, string> | undefined),
    };
    if (token) headers['Authorization'] = `Bearer ${token}`;

    let res: Response;
    try {
        res = await fetch(url, { ...init, headers });
    } catch (ex) {
        const code = typeof ex === 'object' && ex && 'cause' in ex
            && typeof ex.cause === 'object' && ex.cause && 'code' in ex.cause
            ? String(ex.cause.code)
            : 'network error';
        console.warn(`Backend request failed (${code}): ${url}`);
        throw new BackendApiError(503, 'Backend service is unavailable. Please check that the backend is running and try again.');
    }
    if (!res.ok) {
        let msg = res.statusText;
        try {
            const b = await res.json();
            msg = b.message ?? b.error?.message ?? (typeof b.error === 'string' ? b.error : undefined) ?? msg;
        } catch {}
        throw new BackendApiError(res.status, msg);
    }
    if (res.status === 204) return undefined as unknown as T;
    return res.json() as Promise<T>;
}

function get<T>(url: string, token: string)           { return req<T>(url, { token }); }
function post<T>(url: string, token: string, body?: unknown) { return req<T>(url, { method: 'POST',   token, body: body !== undefined ? JSON.stringify(body) : undefined }); }
function put<T>(url: string, token: string, body?: unknown)  { return req<T>(url, { method: 'PUT',    token, body: body !== undefined ? JSON.stringify(body) : undefined }); }
function del<T>(url: string, token: string)           { return req<T>(url, { method: 'DELETE', token }); }
function qs(params: Record<string, string | number | boolean | undefined | null>) {
    const p = new URLSearchParams();
    for (const [k, v] of Object.entries(params)) if (v != null) p.set(k, String(v));
    const s = p.toString();
    return s ? `?${s}` : '';
}

// ── Shared response types ─────────────────────────────────────────────────────

export interface Paginated<T> {
    data: T[];
    total: number;
    page: number;
    per_page: number;
}

// ═══════════════════════════════════════════════════════════════════════════════
// AUTH  (public — no /api/v1 prefix)
// ═══════════════════════════════════════════════════════════════════════════════

export interface BackendUser {
    id: string;
    username: string;
    email: string;
    display_name?: string;
    is_global_admin: boolean;
}

export interface LoginResponse {
    token: string;
    refresh_token: string;
    user: BackendUser;
}

export const auth = {
    /** POST /auth/login */
    login(username: string, password: string): Promise<LoginResponse> {
        return req(`${BASE}/auth/login`, { method: 'POST', body: JSON.stringify({ username, password }) });
    },
    /** POST /auth/register */
    register(body: { username: string; email: string; password: string; display_name?: string }): Promise<{ id: string; message: string; email_verification_required?: boolean }> {
        return req(`${BASE}/auth/register`, { method: 'POST', body: JSON.stringify(body) });
    },
    /** POST /auth/verify-email  { token } — public, no auth header */
    verifyEmail(token: string): Promise<{ ok: boolean }> {
        return req(`${BASE}/auth/verify-email`, { method: 'POST', body: JSON.stringify({ token }) });
    },
    /** POST /auth/resend-verification  { email } — public */
    resendVerification(email: string): Promise<{ ok: boolean }> {
        return req(`${BASE}/auth/resend-verification`, { method: 'POST', body: JSON.stringify({ email }) });
    },
    /** POST /auth/forgot-password */
    forgotPassword(email: string): Promise<{ message: string }> {
        return req(`${BASE}/auth/forgot-password`, { method: 'POST', body: JSON.stringify({ email }) });
    },
    /** POST /auth/reset-password */
    resetPassword(token: string, new_password: string): Promise<{ message: string }> {
        return req(`${BASE}/auth/reset-password`, { method: 'POST', body: JSON.stringify({ token, new_password }) });
    },
    /** GET /api/v1/auth/me */
    me(token: string): Promise<BackendUser & { subscription?: { status: string; plan_name: string; plan_display_name: string; expires_at: string } | null; created_at: string }> {
        return get(`${V1}/auth/me`, token);
    },
    /** POST /auth/refresh — rotate tokens (public, no auth header needed) */
    refresh(refreshToken: string): Promise<{ token: string; refresh_token: string }> {
        return req(`${BASE}/auth/refresh`, {
            method: 'POST',
            body: JSON.stringify({ refresh_token: refreshToken }),
        });
    },
    /** POST /api/v1/auth/logout */
    logout(token: string): Promise<void> {
        return post(`${V1}/auth/logout`, token);
    },
    /** GET /auth/registration-status */
    registrationStatus(): Promise<{ enabled: boolean; first_boot: boolean }> {
        return req(`${BASE}/auth/registration-status`, { method: 'GET' });
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// PROFILE  (self-service)
// ═══════════════════════════════════════════════════════════════════════════════

export interface ProfileSession {
    id: string;
    user_agent?: string;
    ip_address?: string;
    created_at: string;
    last_used_at: string;
}

export interface SshKey {
    id: string;
    name: string;
    fingerprint: string;
    created_at: string;
}

export const profile = {
    /** GET /api/v1/profile */
    get(token: string): Promise<unknown> {
        return get(`${V1}/profile`, token);
    },
    /** PUT /api/v1/profile  { display_name? } */
    update(token: string, body: { display_name?: string }): Promise<void> {
        return put(`${V1}/profile`, token, body);
    },
    /** POST /api/v1/profile/change-password  { current_password, new_password } */
    changePassword(token: string, current_password: string, new_password: string): Promise<void> {
        return post(`${V1}/profile/change-password`, token, { current_password, new_password });
    },
    /** GET /api/v1/profile/sessions */
    listSessions(token: string): Promise<ProfileSession[]> {
        return get(`${V1}/profile/sessions`, token);
    },
    /** DELETE /api/v1/profile/sessions/all */
    revokeAllSessions(token: string): Promise<void> {
        return del(`${V1}/profile/sessions/all`, token);
    },
    /** DELETE /api/v1/profile/sessions/:id */
    revokeSession(token: string, id: string): Promise<void> {
        return del(`${V1}/profile/sessions/${id}`, token);
    },
    /** GET /api/v1/profile/ssh-keys */
    listSshKeys(token: string): Promise<SshKey[]> {
        return get(`${V1}/profile/ssh-keys`, token);
    },
    /** POST /api/v1/profile/ssh-keys  { name, public_key } */
    addSshKey(token: string, name: string, public_key: string): Promise<{ id: string }> {
        return post(`${V1}/profile/ssh-keys`, token, { name, public_key });
    },
    /** GET /api/v1/profile/ssh-keys/:id */
    getSshKey(token: string, id: string): Promise<SshKey & { public_key: string }> {
        return get(`${V1}/profile/ssh-keys/${id}`, token);
    },
    /** PUT /api/v1/profile/ssh-keys/:id  { name?, public_key? } */
    updateSshKey(token: string, id: string, body: { name?: string; public_key?: string }): Promise<void> {
        return put(`${V1}/profile/ssh-keys/${id}`, token, body);
    },
    /** DELETE /api/v1/profile/ssh-keys/:id */
    deleteSshKey(token: string, id: string): Promise<void> {
        return del(`${V1}/profile/ssh-keys/${id}`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// PLANS & SUBSCRIPTIONS  (user self-service)
// ═══════════════════════════════════════════════════════════════════════════════

export const plans = {
    /** GET /api/v1/plans */
    list(token: string): Promise<unknown[]> {
        return get(`${V1}/plans`, token);
    },
    /** GET /api/v1/plans/:id */
    get(token: string, id: string): Promise<unknown> {
        return get(`${V1}/plans/${id}`, token);
    },
};

export const subscription = {
    /** GET /api/v1/subscription */
    get(token: string): Promise<unknown> {
        return get(`${V1}/subscription`, token);
    },
    /**
     * POST /api/v1/subscription
     * { plan_id, billing_cycle?: 'MONTHLY'|'ANNUALLY', auto_renew?: bool }
     */
    subscribe(token: string, body: { plan_id: string; billing_cycle?: string; auto_renew?: boolean }): Promise<{ subscription_id: string; status: string; expires_at: string }> {
        return post(`${V1}/subscription`, token, body);
    },
    /** DELETE /api/v1/subscription  { reason? } */
    cancel(token: string, reason?: string): Promise<void> {
        return del(`${V1}/subscription?${qs({ reason })}`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// BILLING  (user)
// ═══════════════════════════════════════════════════════════════════════════════

export interface Wallet {
    balance: number;          // Decimal in backend, serialised as number
    currency: string;
    updated_at?: string;
}

export interface Transaction {
    id: string;
    tx_type: string;
    amount: number;
    balance_after: number;
    description?: string;
    ref_id?: string;
    created_at: string;
}

export interface Invoice {
    id: string;
    invoice_no: string;
    period_start: string;
    period_end: string;
    total_amount: number;
    status: string;
    created_at: string;
    issued_at?: string;
    paid_at?: string;
}

export const billing = {
    /** GET /api/v1/billing/wallet */
    wallet(token: string): Promise<Wallet> {
        return get(`${V1}/billing/wallet`, token);
    },
    /** GET /api/v1/billing/transactions  ?tx_type&page&per_page */
    listTransactions(token: string, p?: { tx_type?: string; page?: number; per_page?: number }): Promise<Paginated<Transaction>> {
        return get(`${V1}/billing/transactions${qs({ ...p })}`, token);
    },
    /** GET /api/v1/billing/usage */
    currentUsage(token: string): Promise<{ active_apps: number; active_databases: number; mtd_cost: number; last_snapshot?: { time: string; cpu_mcores: number; mem_mb: number; storage_gb: number; hourly_cost: number } | null }> {
        return get(`${V1}/billing/usage`, token);
    },
    /** GET /api/v1/billing/usage/history  ?from&to&page&per_page */
    usageHistory(token: string, p?: { from?: string; to?: string; page?: number; per_page?: number }): Promise<Paginated<unknown>> {
        return get(`${V1}/billing/usage/history${qs({ ...p })}`, token);
    },
    /** GET /api/v1/billing/invoices  ?status&page&per_page */
    listInvoices(token: string, p?: { status?: string; page?: number; per_page?: number }): Promise<Paginated<Invoice>> {
        return get(`${V1}/billing/invoices${qs({ ...p })}`, token);
    },
    /** GET /api/v1/billing/invoices/:id */
    getInvoice(token: string, id: string): Promise<Invoice & { items?: unknown }> {
        return get(`${V1}/billing/invoices/${id}`, token);
    },
    /** GET /api/v1/billing/overdue */
    overdueStatus(token: string): Promise<{ balance: number; is_overdue: boolean; charges: unknown[] }> {
        return get(`${V1}/billing/overdue`, token);
    },
    /** GET /api/v1/projects/:project_id/network-usage */
    projectNetworkUsage(token: string, project_id: string): Promise<unknown[]> {
        return get(`${V1}/projects/${project_id}/network-usage`, token);
    },
    /** GET /api/v1/billing/topup/config */
    topupConfig(token: string): Promise<{ enabled: boolean; currency: string; topup_amounts: number[] }> {
        return get(`${V1}/billing/topup/config`, token);
    },
    /** POST /api/v1/billing/topup  { amount } */
    createTopup(token: string, amount: number): Promise<{ checkout_url: string; session_id: string }> {
        return post(`${V1}/billing/topup`, token, { amount });
    },
    /** GET /api/v1/billing/topup/history */
    topupHistory(token: string): Promise<Array<{ id: string; session_id: string; amount: number; currency: string; status: string; created_at: string; completed_at?: string }>> {
        return get(`${V1}/billing/topup/history`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// PROJECTS  (user)
// ═══════════════════════════════════════════════════════════════════════════════

export interface Project {
    id: string;
    name: string;
    display_name: string;
    is_active: boolean;
    owner_id: string;
    created_at: string;
}

export interface ProjectMember {
    user_id: string;
    username: string;
    email: string;
    display_name?: string;
    role: string;
    added_at: string;
    added_by?: string;
}

export const projects = {
    /** GET /api/v1/projects */
    list(token: string): Promise<unknown[]> {
        return get(`${V1}/projects`, token);
    },
    /**
     * POST /api/v1/projects
     * { name: string (slug), display_name: string }
     */
    create(token: string, body: { name: string; display_name: string }): Promise<{ id: string; name: string }> {
        return post(`${V1}/projects`, token, body);
    },
    /** GET /api/v1/projects/:id */
    get(token: string, id: string): Promise<unknown> {
        return get(`${V1}/projects/${id}`, token);
    },
    /** PUT /api/v1/projects/:id  { display_name? } */
    update(token: string, id: string, body: { display_name?: string }): Promise<void> {
        return put(`${V1}/projects/${id}`, token, body);
    },
    /** DELETE /api/v1/projects/:id */
    delete(token: string, id: string): Promise<void> {
        return del(`${V1}/projects/${id}`, token);
    },
    /** POST /api/v1/projects/:id/leave */
    leave(token: string, id: string): Promise<void> {
        return post(`${V1}/projects/${id}/leave`, token);
    },
    /** POST /api/v1/projects/:id/transfer  { new_owner_id } */
    transferOwner(token: string, id: string, new_owner_id: string): Promise<void> {
        return post(`${V1}/projects/${id}/transfer`, token, { new_owner_id });
    },
    /** GET /api/v1/projects/:id/members */
    listMembers(token: string, id: string): Promise<ProjectMember[]> {
        return get(`${V1}/projects/${id}/members`, token);
    },
    /** POST /api/v1/projects/:id/members  { user_id?, username?, role } */
    addMember(token: string, id: string, body: { user_id?: string; username?: string; role: string }): Promise<void> {
        return post(`${V1}/projects/${id}/members`, token, body);
    },
    /** PUT /api/v1/projects/:id/members/:user_id  { role } */
    updateMember(token: string, id: string, user_id: string, role: string): Promise<void> {
        return put(`${V1}/projects/${id}/members/${user_id}`, token, { role });
    },
    /** DELETE /api/v1/projects/:id/members/:user_id */
    removeMember(token: string, id: string, user_id: string): Promise<void> {
        return del(`${V1}/projects/${id}/members/${user_id}`, token);
    },
    /** GET /api/v1/projects/:project_id/quota */
    quota(token: string, project_id: string): Promise<unknown> {
        return get(`${V1}/projects/${project_id}/quota`, token);
    },
    /** GET /api/v1/projects/:project_id/quota/violations */
    quotaViolations(token: string, project_id: string): Promise<unknown[]> {
        return get(`${V1}/projects/${project_id}/quota/violations`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// APPS
// ═══════════════════════════════════════════════════════════════════════════════

/** Matches CreateAppRequest in apps.rs exactly */
export interface CreateAppRequest {
    name: string;
    display_name?: string;
    pool_id?: string;
    source_type: string;                    // 'IMAGE' | 'GIT'
    container_image?: string;
    container_registry_user?: string;
    container_registry_pass?: string;
    git_url?: string;
    git_branch?: string;
    git_token?: string;
    dockerfile_path?: string;
    container_command?: string;
    container_args?: string[];
    working_dir?: string;
    replicas?: number;
    cpu_reservation_mcores?: number;
    cpu_limit_mcores?: number;
    mem_reservation_mb?: number;
    mem_limit_mb?: number;
    run_as_user?: number;
    run_as_group?: number;
    fs_group?: number;
    privileged?: boolean;
    read_only_root_fs?: boolean;
    gpu_enabled?: boolean;
    gpu_count?: number;
    mount_ldap_files?: boolean;
    mount_etc_hosts?: boolean;
    mount_user_home?: boolean;
    mount_app_data?: boolean;
    mount_app_logs?: boolean;
    timezone?: string;
    anti_affinity_enabled?: boolean;
    health_check_type?: string;
    health_check_path?: string;
    health_check_port?: number;
    health_check_scheme?: string;
    health_check_period?: number;
    health_check_timeout?: number;
    health_check_failures?: number;
    network_policy?: string;
    ingress_network_policy?: string;
    egress_network_policy?: string;
    use_network_policy?: boolean;
}

/** All fields optional — matches UpdateAppRequest in apps.rs */
export type UpdateAppRequest = Omit<Partial<CreateAppRequest>, 'name' | 'source_type'>;

export interface AppEnvVar {
    id: string;
    key: string;
    value?: string;           // null when is_secret=true and caller is OBSERVER
    is_secret: boolean;
}

export interface AppPort {
    id: string;
    container_port: number;
    protocol: string;
    nodeport?: number;
}

export interface AppFileMount {
    id: string;
    filename: string;
    mount_path: string;
    content?: string;         // only present on get_file_mount
}

export interface AppExtraVolume {
    id: string;
    host_path: string;
    mount_path: string;
    read_only: boolean;
}

export interface DeploymentEvent {
    id: string;
    event_type: string;
    status: string;
    message?: string;
    triggered_by?: string;
    created_at: string;
}

/** Minimal domain shape returned by get_by_id */
export interface AppDomain {
    id: string;
    hostname: string;
    ssl_enabled: boolean;
}

/** Shape returned by GET /api/v1/apps/:app_id */
export interface BackendApp {
    id: string;
    project_id: string;
    project_name: string;
    name: string;
    display_name?: string;
    source_type: string;
    app_type?: string;
    container_image?: string;
    git_url?: string;
    git_branch?: string;
    replicas: number;
    status: string;
    paused_at?: string;
    ingress_network_policy?: string;
    egress_network_policy?: string;
    use_network_policy: boolean;
    webhook_id?: string;
    app_domains: AppDomain[];
    created_at: string;
    updated_at: string;
}

export const apps = {
    /** GET /api/v1/projects/:project_id/apps */
    list(token: string, project_id: string): Promise<unknown[]> {
        return get(`${V1}/projects/${project_id}/apps`, token);
    },
    /** POST /api/v1/projects/:project_id/apps */
    create(token: string, project_id: string, body: CreateAppRequest): Promise<{ id: string; webhook_id: string }> {
        return post(`${V1}/projects/${project_id}/apps`, token, body);
    },
    /** POST /api/v1/projects/:project_id/apps/from-template */
    createFromTemplate(token: string, project_id: string, body: TemplateDeployRequest): Promise<{ id: string }> {
        return post(`${V1}/projects/${project_id}/apps/from-template`, token, body);
    },
    /** GET /api/v1/projects/:project_id/managed-usage — P2c usage panel */
    managedUsage(token: string, project_id: string): Promise<{
        db_instances:   { used: number; limit: number };
        mq_bindings:    { used: number; limit: number };
        smtp_bindings:  { used: number; limit: number };
        redis_bindings: { used: number; limit: number };
        s3_bindings:    { used: number; limit: number };
    }> {
        return get(`${V1}/projects/${project_id}/managed-usage`, token);
    },
    /** GET /api/v1/apps/:app_id — fetch by ID only, resolves project internally */
    getById(token: string, app_id: string): Promise<BackendApp> {
        return get(`${V1}/apps/${app_id}`, token);
    },
    /** GET /api/v1/projects/:project_id/apps/:app_id */
    get(token: string, project_id: string, app_id: string): Promise<unknown> {
        return get(`${V1}/projects/${project_id}/apps/${app_id}`, token);
    },
    /** PUT /api/v1/projects/:project_id/apps/:app_id */
    update(token: string, project_id: string, app_id: string, body: UpdateAppRequest): Promise<void> {
        return put(`${V1}/projects/${project_id}/apps/${app_id}`, token, body);
    },
    /** DELETE /api/v1/projects/:project_id/apps/:app_id */
    delete(token: string, project_id: string, app_id: string): Promise<void> {
        return del(`${V1}/projects/${project_id}/apps/${app_id}`, token);
    },
    /** POST /api/v1/projects/:project_id/apps/:app_id/deploy */
    deploy(token: string, project_id: string, app_id: string): Promise<{ status: string }> {
        return post(`${V1}/projects/${project_id}/apps/${app_id}/deploy`, token);
    },
    /** POST /api/v1/projects/:project_id/apps/:app_id/pause  { reason? } */
    pause(token: string, project_id: string, app_id: string, reason?: string): Promise<void> {
        return post(`${V1}/projects/${project_id}/apps/${app_id}/pause`, token, { reason });
    },
    /** POST /api/v1/projects/:project_id/apps/:app_id/resume */
    resume(token: string, project_id: string, app_id: string): Promise<void> {
        return post(`${V1}/projects/${project_id}/apps/${app_id}/resume`, token);
    },
    /** POST /api/v1/projects/:project_id/apps/:app_id/scale  { replicas } */
    scale(token: string, project_id: string, app_id: string, replicas: number): Promise<void> {
        return post(`${V1}/projects/${project_id}/apps/${app_id}/scale`, token, { replicas });
    },
    /** GET /api/v1/projects/:project_id/apps/:app_id/logs  — returns SSE URL, use EventSource */
    logsUrl(project_id: string, app_id: string, token?: string): string {
        const base = `${backendBase(process.env.NEXT_PUBLIC_BACKEND_URL)}/api/v1/projects/${project_id}/apps/${app_id}/logs`;
        return token ? `${base}?token=${encodeURIComponent(token)}` : base;
    },
    /** GET /api/v1/projects/:project_id/apps/:app_id/builds/:build_id/logs */
    buildLogsUrl(project_id: string, app_id: string, build_id: string, token?: string): string {
        const base = `${backendBase(process.env.NEXT_PUBLIC_BACKEND_URL)}/api/v1/projects/${project_id}/apps/${app_id}/builds/${build_id}/logs`;
        return token ? `${base}?token=${encodeURIComponent(token)}` : base;
    },
    /** GET /api/v1/projects/:project_id/apps/:app_id/metrics */
    metricsCurrent(token: string, project_id: string, app_id: string): Promise<unknown> {
        return get(`${V1}/projects/${project_id}/apps/${app_id}/metrics`, token);
    },
    /** GET /api/v1/projects/:project_id/apps/:app_id/metrics/history  ?metric&range&step */
    metricsHistory(token: string, project_id: string, app_id: string, p: { metric: string; range?: string; step?: number }): Promise<unknown> {
        return get(`${V1}/projects/${project_id}/apps/${app_id}/metrics/history${qs(p)}`, token);
    },
    /** GET /api/v1/projects/:project_id/apps/:app_id/events */
    listEvents(token: string, project_id: string, app_id: string): Promise<DeploymentEvent[]> {
        return get(`${V1}/projects/${project_id}/apps/${app_id}/events`, token);
    },

    // ── Env vars ───────────────────────────────────────────────────────────────
    /** GET /api/v1/projects/:project_id/apps/:app_id/env */
    listEnv(token: string, project_id: string, app_id: string): Promise<AppEnvVar[]> {
        return get(`${V1}/projects/${project_id}/apps/${app_id}/env`, token);
    },
    /** POST /api/v1/projects/:project_id/apps/:app_id/env  { key, value, is_secret? } */
    setEnv(token: string, project_id: string, app_id: string, body: { key: string; value: string; is_secret?: boolean }): Promise<void> {
        return post(`${V1}/projects/${project_id}/apps/${app_id}/env`, token, body);
    },
    /** DELETE /api/v1/projects/:project_id/apps/:app_id/env/:env_id */
    deleteEnv(token: string, project_id: string, app_id: string, env_id: string): Promise<void> {
        return del(`${V1}/projects/${project_id}/apps/${app_id}/env/${env_id}`, token);
    },

    // ── Ports ──────────────────────────────────────────────────────────────────
    /** GET /api/v1/projects/:project_id/apps/:app_id/ports */
    listPorts(token: string, project_id: string, app_id: string): Promise<AppPort[]> {
        return get(`${V1}/projects/${project_id}/apps/${app_id}/ports`, token);
    },
    /** POST /api/v1/projects/:project_id/apps/:app_id/ports  { container_port, protocol? } */
    addPort(token: string, project_id: string, app_id: string, body: { container_port: number; protocol?: string }): Promise<{ id: string }> {
        return post(`${V1}/projects/${project_id}/apps/${app_id}/ports`, token, body);
    },
    /** DELETE /api/v1/projects/:project_id/apps/:app_id/ports/:port_id */
    deletePort(token: string, project_id: string, app_id: string, port_id: string): Promise<void> {
        return del(`${V1}/projects/${project_id}/apps/${app_id}/ports/${port_id}`, token);
    },

    // ── File mounts ────────────────────────────────────────────────────────────
    /** GET /api/v1/projects/:project_id/apps/:app_id/files */
    listFiles(token: string, project_id: string, app_id: string): Promise<AppFileMount[]> {
        return get(`${V1}/projects/${project_id}/apps/${app_id}/files`, token);
    },
    /** POST /api/v1/projects/:project_id/apps/:app_id/files  { filename, mount_path, content } */
    setFile(token: string, project_id: string, app_id: string, body: { filename: string; mount_path: string; content: string }): Promise<{ id: string }> {
        return post(`${V1}/projects/${project_id}/apps/${app_id}/files`, token, body);
    },
    /** GET /api/v1/projects/:project_id/apps/:app_id/files/:file_id */
    getFile(token: string, project_id: string, app_id: string, file_id: string): Promise<AppFileMount> {
        return get(`${V1}/projects/${project_id}/apps/${app_id}/files/${file_id}`, token);
    },
    /** DELETE /api/v1/projects/:project_id/apps/:app_id/files/:file_id */
    deleteFile(token: string, project_id: string, app_id: string, file_id: string): Promise<void> {
        return del(`${V1}/projects/${project_id}/apps/${app_id}/files/${file_id}`, token);
    },

    // ── Extra volumes (hostPath) ───────────────────────────────────────────────
    /** GET /api/v1/projects/:project_id/apps/:app_id/volumes */
    listVolumes(token: string, project_id: string, app_id: string): Promise<AppExtraVolume[]> {
        return get(`${V1}/projects/${project_id}/apps/${app_id}/volumes`, token);
    },
    /** POST /api/v1/projects/:project_id/apps/:app_id/volumes  { host_path, mount_path, read_only? } */
    addVolume(token: string, project_id: string, app_id: string, body: { host_path: string; mount_path: string; read_only?: boolean }): Promise<{ id: string }> {
        return post(`${V1}/projects/${project_id}/apps/${app_id}/volumes`, token, body);
    },
    /** DELETE /api/v1/projects/:project_id/apps/:app_id/volumes/:vol_id */
    deleteVolume(token: string, project_id: string, app_id: string, vol_id: string): Promise<void> {
        return del(`${V1}/projects/${project_id}/apps/${app_id}/volumes/${vol_id}`, token);
    },

    // ── Domains ────────────────────────────────────────────────────────────────
    /** GET /api/v1/projects/:project_id/apps/:app_id/domains */
    listDomains(token: string, project_id: string, app_id: string): Promise<unknown[]> {
        return get(`${V1}/projects/${project_id}/apps/${app_id}/domains`, token);
    },
    /** POST /api/v1/projects/:project_id/apps/:app_id/domains  { hostname, target_port, ssl_enabled? } */
    addDomain(token: string, project_id: string, app_id: string, body: { hostname: string; target_port: number; ssl_enabled?: boolean }): Promise<unknown> {
        return post(`${V1}/projects/${project_id}/apps/${app_id}/domains`, token, body);
    },
    /** DELETE /api/v1/projects/:project_id/apps/:app_id/domains/:domain_id */
    deleteDomain(token: string, project_id: string, app_id: string, domain_id: string): Promise<void> {
        return del(`${V1}/projects/${project_id}/apps/${app_id}/domains/${domain_id}`, token);
    },

    // ── Backups ────────────────────────────────────────────────────────────────
    /** GET /api/v1/projects/:project_id/apps/:app_id/backups */
    listBackups(token: string, project_id: string, app_id: string): Promise<unknown[]> {
        return get(`${V1}/projects/${project_id}/apps/${app_id}/backups`, token);
    },
    /** POST /api/v1/projects/:project_id/apps/:app_id/backups  { s3_target_id, cron_expr, retention_days?, backup_type?, db_instance_id? } */
    createBackup(token: string, project_id: string, app_id: string, body: { s3_target_id: string; cron_expr: string; retention_days?: number; backup_type?: string; db_instance_id?: string }): Promise<{ id: string }> {
        return post(`${V1}/projects/${project_id}/apps/${app_id}/backups`, token, body);
    },
    /** PUT /api/v1/projects/:project_id/apps/:app_id/backups/:backup_id  { cron_expr?, retention_days?, is_active? } */
    updateBackup(token: string, project_id: string, app_id: string, backup_id: string, body: { cron_expr?: string; retention_days?: number; is_active?: boolean }): Promise<void> {
        return put(`${V1}/projects/${project_id}/apps/${app_id}/backups/${backup_id}`, token, body);
    },
    /** DELETE /api/v1/projects/:project_id/apps/:app_id/backups/:backup_id */
    deleteBackup(token: string, project_id: string, app_id: string, backup_id: string): Promise<void> {
        return del(`${V1}/projects/${project_id}/apps/${app_id}/backups/${backup_id}`, token);
    },

    // ── Builds  (GIT source only) ──────────────────────────────────────────────
    /** GET /api/v1/projects/:project_id/apps/:app_id/builds */
    listBuilds(token: string, project_id: string, app_id: string): Promise<unknown[]> {
        return get(`${V1}/projects/${project_id}/apps/${app_id}/builds`, token);
    },
    /** POST /api/v1/projects/:project_id/apps/:app_id/builds  { branch? } */
    triggerBuild(token: string, project_id: string, app_id: string, branch?: string): Promise<{ id: string; k8s_job_name: string; status: string }> {
        return post(`${V1}/projects/${project_id}/apps/${app_id}/builds`, token, { branch });
    },
    /** GET /api/v1/projects/:project_id/apps/:app_id/builds/:build_id */
    getBuild(token: string, project_id: string, app_id: string, build_id: string): Promise<unknown> {
        return get(`${V1}/projects/${project_id}/apps/${app_id}/builds/${build_id}`, token);
    },
    /** DELETE /api/v1/projects/:project_id/apps/:app_id/builds/:build_id */
    cancelBuild(token: string, project_id: string, app_id: string, build_id: string): Promise<void> {
        return del(`${V1}/projects/${project_id}/apps/${app_id}/builds/${build_id}`, token);
    },
    // ── Network ────────────────────────────────────────────────────────────────
    /** GET /api/v1/projects/:project_id/apps/:app_id/network */
    getNetwork(token: string, project_id: string, app_id: string): Promise<unknown> {
        return get(`${V1}/projects/${project_id}/apps/${app_id}/network`, token);
    },
    /** DELETE /api/v1/projects/:project_id/apps/:app_id/network */
    releaseNetwork(token: string, project_id: string, app_id: string): Promise<void> {
        return del(`${V1}/projects/${project_id}/apps/${app_id}/network`, token);
    },
    /** POST /api/v1/projects/:project_id/apps/:app_id/network/reassign */
    reassignNetwork(token: string, project_id: string, app_id: string, body: unknown): Promise<unknown> {
        return post(`${V1}/projects/${project_id}/apps/${app_id}/network/reassign`, token, body);
    },

    // ── New endpoints (A5) ────────────────────────────────────────────────────
    /** GET /api/v1/projects/:project_id/apps/:app_id/pods */
    pods(token: string, project_id: string, app_id: string): Promise<unknown[]> {
        return get(`${V1}/projects/${project_id}/apps/${app_id}/pods`, token);
    },
    /** GET /api/v1/projects/:project_id/apps/:app_id/deployments */
    deployments(token: string, project_id: string, app_id: string): Promise<unknown[]> {
        return get(`${V1}/projects/${project_id}/apps/${app_id}/deployments`, token);
    },
    /** POST /api/v1/projects/:project_id/apps/:app_id/webhook/regenerate */
    webhookRegenerate(token: string, project_id: string, app_id: string): Promise<{ webhook_id: string }> {
        return post(`${V1}/projects/${project_id}/apps/${app_id}/webhook/regenerate`, token);
    },
    /** GET /api/v1/projects/:project_id/apps/:app_id/db-credentials */
    dbCredentials(token: string, project_id: string, app_id: string): Promise<unknown> {
        return get(`${V1}/projects/${project_id}/apps/${app_id}/db-credentials`, token);
    },
    // Sub-namespaces for convenience (match actions naming)
    env: {
        list(token: string, project_id: string, app_id: string) {
            return get(`${V1}/projects/${project_id}/apps/${app_id}/env`, token);
        },
        set(token: string, project_id: string, app_id: string, body: { key: string; value: string; is_secret?: boolean }) {
            return post(`${V1}/projects/${project_id}/apps/${app_id}/env`, token, body);
        },
        delete(token: string, project_id: string, app_id: string, env_id: string) {
            return del(`${V1}/projects/${project_id}/apps/${app_id}/env/${env_id}`, token);
        },
    },
    ports: {
        list(token: string, project_id: string, app_id: string) {
            return get(`${V1}/projects/${project_id}/apps/${app_id}/ports`, token);
        },
        add(token: string, project_id: string, app_id: string, body: { port?: number; container_port?: number; protocol?: string; description?: string }) {
            return post(`${V1}/projects/${project_id}/apps/${app_id}/ports`, token, body);
        },
        delete(token: string, project_id: string, app_id: string, port_id: string) {
            return del(`${V1}/projects/${project_id}/apps/${app_id}/ports/${port_id}`, token);
        },
    },
    domains: {
        list(token: string, project_id: string, app_id: string) {
            return get(`${V1}/projects/${project_id}/apps/${app_id}/domains`, token);
        },
        create(token: string, project_id: string, app_id: string, body: { hostname: string; redirect_https?: boolean; use_lets_encrypt?: boolean; basic_auth_username?: string; basic_auth_password?: string }) {
            return post(`${V1}/projects/${project_id}/apps/${app_id}/domains`, token, body);
        },
        delete(token: string, project_id: string, app_id: string, domain_id: string) {
            return del(`${V1}/projects/${project_id}/apps/${app_id}/domains/${domain_id}`, token);
        },
    },
    metrics: {
        current(token: string, project_id: string, app_id: string) {
            return get(`${V1}/projects/${project_id}/apps/${app_id}/metrics`, token);
        },
        history(token: string, project_id: string, app_id: string, p: { metric: string; range?: string; step?: number }) {
            return get(`${V1}/projects/${project_id}/apps/${app_id}/metrics/history${qs(p)}`, token);
        },
    },
    webhook: {
        regenerate(token: string, project_id: string, app_id: string): Promise<{ webhook_id: string }> {
            return post(`${V1}/projects/${project_id}/apps/${app_id}/webhook/regenerate`, token);
        },
    },
    basicAuth: {
        /** GET /api/v1/projects/:project_id/apps/:app_id/basic-auth */
        get(token: string, project_id: string, app_id: string): Promise<{ id: string; username: string } | null> {
            return get(`${V1}/projects/${project_id}/apps/${app_id}/basic-auth`, token);
        },
        /** PUT /api/v1/projects/:project_id/apps/:app_id/basic-auth */
        set(token: string, project_id: string, app_id: string, body: { username: string; password: string }): Promise<void> {
            return put(`${V1}/projects/${project_id}/apps/${app_id}/basic-auth`, token, body);
        },
        /** DELETE /api/v1/projects/:project_id/apps/:app_id/basic-auth */
        delete(token: string, project_id: string, app_id: string): Promise<void> {
            return del(`${V1}/projects/${project_id}/apps/${app_id}/basic-auth`, token);
        },
    },
    managedVolumes: {
        /** GET /api/v1/projects/:project_id/apps/:app_id/managed-volumes */
        list(token: string, project_id: string, app_id: string): Promise<unknown[]> {
            return get(`${V1}/projects/${project_id}/apps/${app_id}/managed-volumes`, token);
        },
        /** POST /api/v1/projects/:project_id/apps/:app_id/managed-volumes */
        create(token: string, project_id: string, app_id: string, body: { name: string; container_mount_path: string; share_with_others?: boolean; shared_volume_id?: string }): Promise<{ id: string; host_path: string }> {
            return post(`${V1}/projects/${project_id}/apps/${app_id}/managed-volumes`, token, body);
        },
        /** PUT /api/v1/projects/:project_id/apps/:app_id/managed-volumes/:vid */
        update(token: string, project_id: string, app_id: string, vol_id: string, body: { name?: string; container_mount_path?: string; share_with_others?: boolean }): Promise<void> {
            return put(`${V1}/projects/${project_id}/apps/${app_id}/managed-volumes/${vol_id}`, token, body);
        },
        /** DELETE /api/v1/projects/:project_id/apps/:app_id/managed-volumes/:vid */
        delete(token: string, project_id: string, app_id: string, vol_id: string): Promise<void> {
            return del(`${V1}/projects/${project_id}/apps/${app_id}/managed-volumes/${vol_id}`, token);
        },
        /** GET /api/v1/projects/:project_id/managed-volumes/shareable?excludeAppId= */
        shareable(token: string, project_id: string, exclude_app_id?: string): Promise<unknown[]> {
            return get(`${V1}/projects/${project_id}/managed-volumes/shareable${qs({ excludeAppId: exclude_app_id })}`, token);
        },
        /** GET /api/v1/projects/:project_id/apps/:app_id/managed-volumes/:vid/usage */
        usage(token: string, project_id: string, app_id: string, vol_id: string): Promise<{ host_path: string; usage_bytes: number }> {
            return get(`${V1}/projects/${project_id}/apps/${app_id}/managed-volumes/${vol_id}/usage`, token);
        },
        backups: {
            /** GET .../managed-volumes/:vid/backups */
            list(token: string, project_id: string, app_id: string, vol_id: string): Promise<unknown[]> {
                return get(`${V1}/projects/${project_id}/apps/${app_id}/managed-volumes/${vol_id}/backups`, token);
            },
            /** POST .../managed-volumes/:vid/backups */
            create(token: string, project_id: string, app_id: string, vol_id: string, body: { s3_target_id: string; cron_expr: string; retention_days?: number; use_db_backup?: boolean }): Promise<{ id: string }> {
                return post(`${V1}/projects/${project_id}/apps/${app_id}/managed-volumes/${vol_id}/backups`, token, body);
            },
            /** PUT .../managed-volumes/:vid/backups/:bid */
            update(token: string, project_id: string, app_id: string, vol_id: string, bid: string, body: { cron_expr?: string; retention_days?: number; is_active?: boolean }): Promise<void> {
                return put(`${V1}/projects/${project_id}/apps/${app_id}/managed-volumes/${vol_id}/backups/${bid}`, token, body);
            },
            /** DELETE .../managed-volumes/:vid/backups/:bid */
            delete(token: string, project_id: string, app_id: string, vol_id: string, bid: string): Promise<void> {
                return del(`${V1}/projects/${project_id}/apps/${app_id}/managed-volumes/${vol_id}/backups/${bid}`, token);
            },
            /** POST .../managed-volumes/:vid/backups/:bid/run */
            run(token: string, project_id: string, app_id: string, vol_id: string, bid: string): Promise<{ status: string }> {
                return post(`${V1}/projects/${project_id}/apps/${app_id}/managed-volumes/${vol_id}/backups/${bid}/run`, token);
            },
        },
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// DATABASES  (user)
// ═══════════════════════════════════════════════════════════════════════════════

export const databases = {
    /** GET /api/v1/projects/:project_id/databases */
    list(token: string, project_id: string): Promise<unknown[]> {
        return get(`${V1}/projects/${project_id}/databases`, token);
    },
    /** POST /api/v1/projects/:project_id/databases  { cluster_id, name } */
    create(token: string, project_id: string, body: { cluster_id: string; name: string }): Promise<{ id: string; secret_name: string }> {
        return post(`${V1}/projects/${project_id}/databases`, token, body);
    },
    /** GET /api/v1/database-clusters — tenant-safe (no admin creds) */
    listClusters(token: string): Promise<Array<{ id: string; name: string; cluster_type: string; description?: string }>> {
        return get(`${V1}/database-clusters`, token);
    },
    /** GET /api/v1/projects/:project_id/databases/:db_id */
    get(token: string, project_id: string, db_id: string): Promise<unknown> {
        return get(`${V1}/projects/${project_id}/databases/${db_id}`, token);
    },
    /** DELETE /api/v1/projects/:project_id/databases/:db_id */
    delete(token: string, project_id: string, db_id: string): Promise<void> {
        return del(`${V1}/projects/${project_id}/databases/${db_id}`, token);
    },
    /** GET /api/v1/projects/:project_id/databases/:db_id/credentials */
    credentials(token: string, project_id: string, db_id: string): Promise<{ host: string; port: number; database: string; username: string; password: string }> {
        return get(`${V1}/projects/${project_id}/databases/${db_id}/credentials`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// S3 TARGETS  (user pick list)
// ═══════════════════════════════════════════════════════════════════════════════

export const s3Targets = {
    /** GET /api/v1/s3-targets  — active targets visible to users */
    list(token: string): Promise<Array<{ id: string; name: string; endpoint: string; region?: string; bucket_name: string }>> {
        return get(`${V1}/s3-targets`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// NETWORK / IPAM  (user)
// ═══════════════════════════════════════════════════════════════════════════════

export const network = {
    /** GET /api/v1/projects/:project_id/network/pools */
    listProjectPools(token: string, project_id: string): Promise<unknown[]> {
        return get(`${V1}/projects/${project_id}/network/pools`, token);
    },
    /** GET /api/v1/projects/:project_id/network/allocations */
    listProjectAllocations(token: string, project_id: string): Promise<unknown[]> {
        return get(`${V1}/projects/${project_id}/network/allocations`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// MONITORING — Aggregate endpoints
// ═══════════════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════════════
// BACKUPS — Aggregate listing
// ═══════════════════════════════════════════════════════════════════════════════

export const backups = {
    /** GET /api/v1/backups?s3_target_id= — all accessible backup schedules */
    list(token: string, s3TargetId?: string): Promise<Array<{
        id: string; type: 'app' | 'volume';
        app_id: string; app_name: string; project_id: string; project_name: string;
        s3_target_id: string; s3_target_name: string;
        cron_expr: string; retention_days?: number; is_active: boolean;
        backup_type?: string;
        volume_id?: string; volume_name?: string; mount_path?: string;
    }>> {
        const params = s3TargetId ? `?s3_target_id=${encodeURIComponent(s3TargetId)}` : '';
        return get(`${V1}/backups${params}`, token);
    },
    /** DELETE /api/v1/backups/:s3_target_id/file?key= — delete backup file (admin only) */
    deleteFile(token: string, s3TargetId: string, key: string): Promise<void> {
        return del(`${V1}/backups/${s3TargetId}/file?key=${encodeURIComponent(key)}`, token);
    },
    /** GET /api/v1/backups/:s3_target_id/download?key= — presign download (admin only) */
    download(token: string, s3TargetId: string, key: string): Promise<unknown> {
        return get(`${V1}/backups/${s3TargetId}/download?key=${encodeURIComponent(key)}`, token);
    },
};

export const monitoring = {
    /** GET /api/v1/monitoring/apps — all accessible apps with latest CPU/RAM metrics */
    apps(token: string): Promise<Array<{
        app_id: string; app_name: string; project_id: string; project_name: string;
        status: string; replicas: number; cpu_mcores: number; ram_bytes: number;
    }>> {
        return get(`${V1}/monitoring/apps`, token);
    },
    /** GET /api/v1/monitoring/managed-volumes — all accessible managed volumes with usage info */
    managedVolumes(token: string): Promise<Array<{
        id: string; name: string; container_mount_path: string; host_path?: string;
        app_id: string; app_name: string; project_id: string; project_name: string;
        usage_bytes: number | null;
    }>> {
        return get(`${V1}/monitoring/managed-volumes`, token);
    },
    /** GET /api/v1/admin/nodes/metrics/aggregate — cluster-wide resource totals (admin only) */
    nodesAggregate(token: string): Promise<{
        node_count: number;
        total_cpu_capacity_mcores: number;
        avg_cpu_used_pct: number;
        total_mem_capacity_mb: number;
        total_mem_used_bytes: number;
        total_mem_total_bytes: number;
        total_disk_used_bytes: number;
        total_disk_total_bytes: number;
    }> {
        return get(`${V1}/admin/nodes/metrics/aggregate`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// ADMIN — USERS
// ═══════════════════════════════════════════════════════════════════════════════

export const adminUsers = {
    /** GET /api/v1/admin/users  ?search&is_active&page&per_page */
    list(token: string, p?: { search?: string; is_active?: boolean; page?: number; per_page?: number }): Promise<Paginated<unknown>> {
        return get(`${V1}/admin/users${qs({ ...p })}`, token);
    },
    /** POST /api/v1/admin/users  { username, email, password, display_name?, is_global_admin? } */
    create(token: string, body: { username: string; email: string; password: string; display_name?: string; is_global_admin?: boolean }): Promise<{ id: string }> {
        return post(`${V1}/admin/users`, token, body);
    },
    /** GET /api/v1/admin/users/:id */
    get(token: string, id: string): Promise<unknown> {
        return get(`${V1}/admin/users/${id}`, token);
    },
    /** PUT /api/v1/admin/users/:id  { username?, display_name?, email?, is_active?, is_global_admin? } */
    update(token: string, id: string, body: { username?: string; display_name?: string; email?: string; is_active?: boolean; is_global_admin?: boolean }): Promise<void> {
        return put(`${V1}/admin/users/${id}`, token, body);
    },
    /** DELETE /api/v1/admin/users/:id */
    delete(token: string, id: string): Promise<void> {
        return del(`${V1}/admin/users/${id}`, token);
    },
    /** POST /api/v1/admin/users/:id/reset-password  { new_password } */
    resetPassword(token: string, id: string, new_password: string): Promise<void> {
        return post(`${V1}/admin/users/${id}/reset-password`, token, { new_password });
    },
    /** GET /api/v1/admin/users/:id/usage */
    usage(token: string, id: string): Promise<{ active_apps: number; active_databases: number; last_snapshot?: unknown }> {
        return get(`${V1}/admin/users/${id}/usage`, token);
    },
    /** GET /api/v1/admin/users/:id/subscription */
    getSubscription(token: string, id: string): Promise<unknown> {
        return get(`${V1}/admin/users/${id}/subscription`, token);
    },
    /** POST /api/v1/admin/users/:id/subscription  (assign plan) */
    assignPlan(token: string, id: string, body: unknown): Promise<unknown> {
        return post(`${V1}/admin/users/${id}/subscription`, token, body);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// ADMIN — LDAP
// ═══════════════════════════════════════════════════════════════════════════════

export interface LdapSyncReport {
    scanned: number;
    inserted: number;
    updated: number;
    skipped: number;
    conflicts: Array<{
        ldap_dn: string;
        ldap_uid: string;
        ldap_email: string;
        reason: string;
        local_matches: Array<{
            id: string;
            username: string;
            email: string;
            ldap_dn?: string | null;
        }>;
    }>;
}

export const adminLdap = {
    /** POST /api/v1/admin/ldap/test-connection */
    testConnection(token: string): Promise<{ ok: boolean; user_count: number }> {
        return post(`${V1}/admin/ldap/test-connection`, token);
    },
    /** POST /api/v1/admin/ldap/sync */
    sync(token: string): Promise<LdapSyncReport> {
        return post(`${V1}/admin/ldap/sync`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// ADMIN — PROJECTS
// ═══════════════════════════════════════════════════════════════════════════════

export const adminProjects = {
    /** GET /api/v1/admin/projects  ?search&is_active&owner_id&page&per_page */
    list(token: string, p?: { search?: string; is_active?: boolean; owner_id?: string; page?: number; per_page?: number }): Promise<Paginated<unknown>> {
        return get(`${V1}/admin/projects${qs({ ...p })}`, token);
    },
    /**
     * POST /api/v1/admin/projects
     * { name, display_name, owner_id, quota_cpu_mcores?, quota_mem_mb?, quota_storage_gb?,
     *   quota_apps?, quota_db_instances?, quota_bandwidth_gb?, quota_domain_count?, quota_request_million? }
     */
    create(token: string, body: {
        name: string; display_name: string; owner_id?: string;
        quota_cpu_mcores?: number; quota_mem_mb?: number; quota_storage_gb?: number;
        quota_apps?: number; quota_db_instances?: number; quota_bandwidth_gb?: number;
        quota_domain_count?: number; quota_request_million?: number;
    }): Promise<{ id: string; name: string }> {
        return post(`${V1}/admin/projects`, token, body);
    },
    /** GET /api/v1/admin/projects/:id */
    get(token: string, id: string): Promise<unknown> {
        return get(`${V1}/admin/projects/${id}`, token);
    },
    /** PUT /api/v1/admin/projects/:id  { display_name?, is_active?, owner_id?, quota_*? } */
    update(token: string, id: string, body: {
        display_name?: string; is_active?: boolean; owner_id?: string;
        quota_cpu_mcores?: number; quota_mem_mb?: number; quota_storage_gb?: number;
        quota_apps?: number; quota_db_instances?: number; quota_bandwidth_gb?: number;
        quota_domain_count?: number; quota_request_million?: number;
    }): Promise<void> {
        return put(`${V1}/admin/projects/${id}`, token, body);
    },
    /** DELETE /api/v1/admin/projects/:id */
    delete(token: string, id: string): Promise<void> {
        return del(`${V1}/admin/projects/${id}`, token);
    },
    /** POST /api/v1/admin/projects/:id/quota/enforce */
    enforceQuota(token: string, id: string, body?: unknown): Promise<unknown> {
        return post(`${V1}/admin/projects/${id}/quota/enforce`, token, body);
    },
    /** POST /api/v1/admin/apps/:app_id/suspend */
    suspendApp(token: string, app_id: string, body?: unknown): Promise<void> {
        return post(`${V1}/admin/apps/${app_id}/suspend`, token, body);
    },
    /** POST /api/v1/admin/apps/:app_id/unsuspend */
    unsuspendApp(token: string, app_id: string): Promise<void> {
        return post(`${V1}/admin/apps/${app_id}/unsuspend`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// ADMIN — BILLING
// ═══════════════════════════════════════════════════════════════════════════════

export const adminBilling = {
    /** GET /api/v1/admin/billing/wallets  ?search&page&per_page */
    listWallets(token: string, p?: { search?: string; page?: number; per_page?: number }): Promise<Paginated<unknown>> {
        return get(`${V1}/admin/billing/wallets${qs({ ...p })}`, token);
    },
    /** GET /api/v1/admin/billing/transactions  ?search&tx_type&page&per_page */
    listTransactions(token: string, p?: { search?: string; tx_type?: string; page?: number; per_page?: number }): Promise<Paginated<unknown>> {
        return get(`${V1}/admin/billing/transactions${qs({ ...p })}`, token);
    },
    /** POST /api/v1/admin/billing/recharge  { user_id, amount, description, idempotency_key? } */
    recharge(token: string, user_id: string, amount: number, description: string, idempotency_key?: string): Promise<{ transaction_id: string; new_balance: number }> {
        return post(`${V1}/admin/billing/recharge`, token, { user_id, amount, description, idempotency_key });
    },
    /** POST /api/v1/admin/billing/adjustment  { user_id, amount, description, idempotency_key? } */
    adjust(token: string, user_id: string, amount: number, description: string, idempotency_key?: string): Promise<{ transaction_id: string; new_balance: number }> {
        return post(`${V1}/admin/billing/adjustment`, token, { user_id, amount, description, idempotency_key });
    },
    /** GET /api/v1/admin/billing/invoices  ?status&page&per_page */
    listInvoices(token: string, p?: { status?: string; page?: number; per_page?: number }): Promise<Paginated<unknown>> {
        return get(`${V1}/admin/billing/invoices${qs({ ...p })}`, token);
    },
    /** POST /api/v1/admin/billing/invoices  { user_id, period_start, period_end } */
    generateInvoice(token: string, body: { user_id: string; period_start: string; period_end: string }): Promise<{ id: string; invoice_no: string; total_amount: number }> {
        return post(`${V1}/admin/billing/invoices`, token, body);
    },
    /** POST /api/v1/admin/billing/invoices/:id/pay */
    markPaid(token: string, id: string): Promise<void> {
        return post(`${V1}/admin/billing/invoices/${id}/pay`, token);
    },
    /** POST /api/v1/admin/billing/invoices/:id/void */
    voidInvoice(token: string, id: string): Promise<void> {
        return post(`${V1}/admin/billing/invoices/${id}/void`, token);
    },
    /** POST /api/v1/admin/billing/network-charges/compute  { billing_month } */
    computeNetworkCharges(token: string, billing_month: string): Promise<{ computed: number; billing_month: string }> {
        return post(`${V1}/admin/billing/network-charges/compute`, token, { billing_month });
    },
    /** POST /api/v1/admin/billing/network-charges/collect  { billing_month } */
    collectNetworkCharges(token: string, billing_month: string): Promise<{ collected_projects: string[]; count: number }> {
        return post(`${V1}/admin/billing/network-charges/collect`, token, { billing_month });
    },
    /** GET /api/v1/admin/billing/overdue */
    listOverdue(token: string): Promise<unknown[]> {
        return get(`${V1}/admin/billing/overdue`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// ADMIN — PLANS & SUBSCRIPTIONS
// ═══════════════════════════════════════════════════════════════════════════════

export const adminPlans = {
    /** GET /api/v1/admin/plans  ?include_inactive&page&per_page */
    list(token: string, p?: { include_inactive?: boolean; page?: number; per_page?: number }): Promise<Paginated<unknown>> {
        return get(`${V1}/admin/plans${qs({ ...p })}`, token);
    },
    /**
     * POST /api/v1/admin/plans
     * { name, display_name, description?, price_monthly, price_annually?,
     *   quota_cpu_mcores, quota_mem_mb, quota_storage_gb, quota_bandwidth_gb,
     *   quota_domain_count, quota_db_instance_count, quota_project_count,
     *   quota_app_count, quota_request_million, is_public?, sort_order? }
     */
    create(token: string, body: {
        name: string; display_name: string; description?: string;
        price_monthly: number; price_annually?: number;
        quota_cpu_mcores: number; quota_mem_mb: number; quota_storage_gb: number;
        quota_bandwidth_gb: number; quota_domain_count: number;
        quota_db_instance_count: number; quota_project_count: number;
        quota_app_count: number; quota_request_million: number;
        is_public?: boolean; sort_order?: number;
    }): Promise<{ id: string }> {
        return post(`${V1}/admin/plans`, token, body);
    },
    /** GET /api/v1/admin/plans/:id */
    get(token: string, id: string): Promise<unknown> {
        return get(`${V1}/admin/plans/${id}`, token);
    },
    /** PUT /api/v1/admin/plans/:id */
    update(token: string, id: string, body: Record<string, unknown>): Promise<void> {
        return put(`${V1}/admin/plans/${id}`, token, body);
    },
    /** DELETE /api/v1/admin/plans/:id */
    delete(token: string, id: string): Promise<void> {
        return del(`${V1}/admin/plans/${id}`, token);
    },
    /** GET /api/v1/admin/subscriptions */
    listSubscriptions(token: string): Promise<unknown[]> {
        return get(`${V1}/admin/subscriptions`, token);
    },
    /** PUT /api/v1/admin/subscriptions/:id */
    updateSubscription(token: string, id: string, body: unknown): Promise<void> {
        return put(`${V1}/admin/subscriptions/${id}`, token, body);
    },
    /** DELETE /api/v1/admin/subscriptions/:id */
    cancelSubscription(token: string, id: string): Promise<void> {
        return del(`${V1}/admin/subscriptions/${id}`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// ADMIN — NODES
// ═══════════════════════════════════════════════════════════════════════════════

export const adminNodes = {
    /** GET /api/v1/admin/nodes */
    list(token: string): Promise<unknown[]> {
        return get(`${V1}/admin/nodes`, token);
    },
    /**
     * POST /api/v1/admin/nodes
     * { cluster_id, hostname, ip_address, node_role?, ssh_password, storage_path? }
     * Returns 202 Accepted — provisioning is async
     */
    add(token: string, body: { cluster_id: string; hostname: string; ip_address: string; node_role?: string; ssh_password?: string; ssh_port?: number; storage_path?: string }): Promise<{ id: string; status: string; cluster_id: string; storage_path: string }> {
        return post(`${V1}/admin/nodes`, token, body);
    },
    /** GET /api/v1/admin/nodes/:id */
    get(token: string, id: string): Promise<unknown> {
        return get(`${V1}/admin/nodes/${id}`, token);
    },
    /** DELETE /api/v1/admin/nodes/:id  — drains then deletes */
    delete(token: string, id: string): Promise<void> {
        return del(`${V1}/admin/nodes/${id}`, token);
    },
    /** PUT /api/v1/admin/nodes/:id/labels  body is a flat JSON object of labels */
    updateLabels(token: string, id: string, labels: Record<string, string>): Promise<void> {
        return put(`${V1}/admin/nodes/${id}/labels`, token, labels);
    },
    /** GET /api/v1/admin/nodes/:id/health */
    health(token: string, id: string): Promise<{ id: string; node_status: string }> {
        return get(`${V1}/admin/nodes/${id}/health`, token);
    },
    /** GET /api/v1/admin/nodes/:id/metrics */
    metrics(token: string, id: string): Promise<{ id: string; hostname: string; cpu: unknown; memory: unknown; disk: unknown; load: unknown; gpu: unknown }> {
        return get(`${V1}/admin/nodes/${id}/metrics`, token);
    },
    /** POST /api/v1/admin/nodes/:id/cordon — mark node unschedulable */
    cordon(token: string, id: string): Promise<{ id: string; schedulable: boolean }> {
        return post(`${V1}/admin/nodes/${id}/cordon`, token);
    },
    /** POST /api/v1/admin/nodes/:id/uncordon — mark node schedulable */
    uncordon(token: string, id: string): Promise<{ id: string; schedulable: boolean }> {
        return post(`${V1}/admin/nodes/${id}/uncordon`, token);
    },
    /** PUT /api/v1/admin/nodes/:id — update node settings */
    update(token: string, id: string, body: { hostname?: string; ip_address?: string; node_role?: string; storage_path?: string; ssh_port?: number }): Promise<unknown> {
        return put(`${V1}/admin/nodes/${id}`, token, body);
    },
    /** POST /api/v1/admin/nodes/:id/reprovision — retry provisioning */
    reprovision(token: string, id: string, body: { ssh_password?: string }): Promise<unknown> {
        return post(`${V1}/admin/nodes/${id}/reprovision`, token, body);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// ADMIN — CLUSTERS  (K3s)
// ═══════════════════════════════════════════════════════════════════════════════

export const adminClusters = {
    /** GET /api/v1/admin/clusters */
    list(token: string): Promise<unknown[]> {
        return get(`${V1}/admin/clusters`, token);
    },
    /**
     * POST /api/v1/admin/clusters
     * { pool_id, name, display_name?, description?, k3s_token?, vpc_pool_id?, pub_pool_id?, node_main_iface? }
     */
    create(token: string, body: { pool_id?: string; name: string; display_name?: string; description?: string; k3s_token?: string; ip_pool_id?: string; node_main_iface?: string; orchestrator?: 'K3S' | 'DOCKER' }): Promise<{ id: string }> {
        return post(`${V1}/admin/clusters`, token, body);
    },
    /** GET /api/v1/admin/clusters/:id */
    get(token: string, id: string): Promise<unknown> {
        return get(`${V1}/admin/clusters/${id}`, token);
    },
    /** PUT /api/v1/admin/clusters/:id  { display_name?, description?, is_active?, ip_pool_id?, node_main_iface? } */
    update(token: string, id: string, body: { display_name?: string; description?: string; is_active?: boolean; ip_pool_id?: string; node_main_iface?: string }): Promise<void> {
        return put(`${V1}/admin/clusters/${id}`, token, body);
    },
    /** DELETE /api/v1/admin/clusters/:id */
    delete(token: string, id: string): Promise<void> {
        return del(`${V1}/admin/clusters/${id}`, token);
    },
    /** GET /api/v1/admin/cluster/storage */
    getStorage(token: string): Promise<unknown> {
        return get(`${V1}/admin/cluster/storage`, token);
    },
    /** PUT /api/v1/admin/cluster/storage */
    updateStorage(token: string, body: unknown): Promise<void> {
        return put(`${V1}/admin/cluster/storage`, token, body);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// ADMIN — RESOURCE POOLS
// ═══════════════════════════════════════════════════════════════════════════════

export const adminPools = {
    /** GET /api/v1/admin/resource-pools */
    list(token: string): Promise<unknown[]> {
        return get(`${V1}/admin/resource-pools`, token);
    },
    /** POST /api/v1/admin/resource-pools  { name, display_name, region?, description? } */
    create(token: string, body: { name: string; display_name: string; region?: string; description?: string }): Promise<unknown> {
        return post(`${V1}/admin/resource-pools`, token, body);
    },
    /** GET /api/v1/admin/resource-pools/:id */
    get(token: string, id: string): Promise<unknown> {
        return get(`${V1}/admin/resource-pools/${id}`, token);
    },
    /** PUT /api/v1/admin/resource-pools/:id  { display_name?, region?, description?, is_active? } */
    update(token: string, id: string, body: { display_name?: string; region?: string; description?: string; is_active?: boolean }): Promise<void> {
        return put(`${V1}/admin/resource-pools/${id}`, token, body);
    },
    /** DELETE /api/v1/admin/resource-pools/:id */
    delete(token: string, id: string): Promise<void> {
        return del(`${V1}/admin/resource-pools/${id}`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// ADMIN — DATABASE CLUSTERS
// ═══════════════════════════════════════════════════════════════════════════════

export const adminDbClusters = {
    /** GET /api/v1/admin/db-clusters */
    list(token: string): Promise<unknown[]> {
        return get(`${V1}/admin/db-clusters`, token);
    },
    /** POST /api/v1/admin/db-clusters  { name, cluster_type, host, port, admin_user, admin_password, max_databases?, description?, manager_url? } */
    create(token: string, body: { name: string; cluster_type: string; host: string; port: number; admin_user: string; admin_password: string; max_databases?: number; description?: string; manager_url?: string }): Promise<{ id: string }> {
        return post(`${V1}/admin/db-clusters`, token, body);
    },
    /** GET /api/v1/admin/db-clusters/:id */
    get(token: string, id: string): Promise<unknown> {
        return get(`${V1}/admin/db-clusters/${id}`, token);
    },
    /** PUT /api/v1/admin/db-clusters/:id  { host?, port?, admin_user?, admin_password?, max_databases?, description?, manager_url?, is_active? } */
    update(token: string, id: string, body: { host?: string; port?: number; admin_user?: string; admin_password?: string; max_databases?: number; description?: string; manager_url?: string; is_active?: boolean }): Promise<void> {
        return put(`${V1}/admin/db-clusters/${id}`, token, body);
    },
    /** DELETE /api/v1/admin/db-clusters/:id */
    delete(token: string, id: string): Promise<void> {
        return del(`${V1}/admin/db-clusters/${id}`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// ADMIN — IMAGE REGISTRIES
// ═══════════════════════════════════════════════════════════════════════════════

export const adminRegistries = {
    /** GET /api/v1/admin/registries */
    list(token: string): Promise<unknown[]> {
        return get(`${V1}/admin/registries`, token);
    },
    /** POST /api/v1/admin/registries  { name, endpoint, username?, password?, is_default?, priority? } */
    create(token: string, body: { name: string; endpoint: string; username?: string; password?: string; is_default?: boolean; priority?: number }): Promise<{ id: string }> {
        return post(`${V1}/admin/registries`, token, body);
    },
    /** GET /api/v1/admin/registries/:id */
    get(token: string, id: string): Promise<unknown> {
        return get(`${V1}/admin/registries/${id}`, token);
    },
    /** PUT /api/v1/admin/registries/:id  { name?, endpoint?, username?, password?, is_default?, priority?, is_active? } */
    update(token: string, id: string, body: { name?: string; endpoint?: string; username?: string; password?: string; is_default?: boolean; priority?: number; is_active?: boolean }): Promise<void> {
        return put(`${V1}/admin/registries/${id}`, token, body);
    },
    /** DELETE /api/v1/admin/registries/:id */
    delete(token: string, id: string): Promise<void> {
        return del(`${V1}/admin/registries/${id}`, token);
    },
    /** GET /api/v1/admin/registries/:id/images */
    listImages(token: string, id: string): Promise<{ images: Array<{ name: string; tags: string[] }> }> {
        return get(`${V1}/admin/registries/${id}/images`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// ADMIN — S3 TARGETS
// ═══════════════════════════════════════════════════════════════════════════════

export const adminS3Targets = {
    /** GET /api/v1/admin/s3-targets */
    list(token: string): Promise<unknown[]> {
        return get(`${V1}/admin/s3-targets`, token);
    },
    /** POST /api/v1/admin/s3-targets  { name, endpoint, region?, access_key_id, secret_key, bucket_name } */
    create(token: string, body: { name: string; endpoint: string; region?: string; access_key_id: string; secret_key: string; bucket_name: string }): Promise<{ id: string }> {
        return post(`${V1}/admin/s3-targets`, token, body);
    },
    /** GET /api/v1/admin/s3-targets/:id */
    get(token: string, id: string): Promise<unknown> {
        return get(`${V1}/admin/s3-targets/${id}`, token);
    },
    /** PUT /api/v1/admin/s3-targets/:id  { name?, endpoint?, region?, access_key_id?, secret_key?, bucket_name?, is_active? } */
    update(token: string, id: string, body: { name?: string; endpoint?: string; region?: string; access_key_id?: string; secret_key?: string; bucket_name?: string; is_active?: boolean }): Promise<void> {
        return put(`${V1}/admin/s3-targets/${id}`, token, body);
    },
    /** DELETE /api/v1/admin/s3-targets/:id */
    delete(token: string, id: string): Promise<void> {
        return del(`${V1}/admin/s3-targets/${id}`, token);
    },
    /** POST /api/v1/admin/s3-targets/test — verify credentials before saving */
    test(token: string, body: { endpoint: string; region?: string; access_key_id: string; secret_key: string; bucket_name: string }): Promise<{ ok: boolean }> {
        return post(`${V1}/admin/s3-targets/test`, token, body);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// ADMIN — PROXY MANAGERS
// ═══════════════════════════════════════════════════════════════════════════════

export const adminProxyManagers = {
    /** GET /api/v1/admin/proxy-managers */
    list(token: string): Promise<unknown[]> {
        return get(`${V1}/admin/proxy-managers`, token);
    },
    /** POST /api/v1/admin/proxy-managers  { name, host, api_base_url, api_password } */
    create(token: string, body: { name: string; host: string; api_base_url: string; api_password: string }): Promise<unknown> {
        return post(`${V1}/admin/proxy-managers`, token, body);
    },
    /** GET /api/v1/admin/proxy-managers/:id */
    get(token: string, id: string): Promise<unknown> {
        return get(`${V1}/admin/proxy-managers/${id}`, token);
    },
    /** PUT /api/v1/admin/proxy-managers/:id  { name?, host?, api_base_url?, api_password?, is_active? } */
    update(token: string, id: string, body: { name?: string; host?: string; api_base_url?: string; api_password?: string; is_active?: boolean }): Promise<void> {
        return put(`${V1}/admin/proxy-managers/${id}`, token, body);
    },
    /** DELETE /api/v1/admin/proxy-managers/:id */
    delete(token: string, id: string): Promise<void> {
        return del(`${V1}/admin/proxy-managers/${id}`, token);
    },
    /** POST /api/v1/admin/proxy-managers/:id/test */
    test(token: string, id: string): Promise<unknown> {
        return post(`${V1}/admin/proxy-managers/${id}/test`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// ADMIN — IP POOLS  (IPAM)
// ═══════════════════════════════════════════════════════════════════════════════

export const adminIpPools = {
    /** GET /api/v1/admin/ip-pools */
    list(token: string): Promise<unknown[]> {
        return get(`${V1}/admin/ip-pools`, token);
    },
    /** POST /api/v1/admin/ip-pools  { name, cidr, pool_type?, gateway?, description? } */
    create(token: string, body: { name: string; cidr: string; pool_type?: string; gateway?: string; description?: string }): Promise<unknown> {
        return post(`${V1}/admin/ip-pools`, token, body);
    },
    /** GET /api/v1/admin/ip-pools/:id */
    get(token: string, id: string): Promise<unknown> {
        return get(`${V1}/admin/ip-pools/${id}`, token);
    },
    /** PUT /api/v1/admin/ip-pools/:id  { name?, gateway?, description?, is_active? } */
    update(token: string, id: string, body: { name?: string; gateway?: string; description?: string; is_active?: boolean }): Promise<void> {
        return put(`${V1}/admin/ip-pools/${id}`, token, body);
    },
    /** DELETE /api/v1/admin/ip-pools/:id */
    delete(token: string, id: string): Promise<void> {
        return del(`${V1}/admin/ip-pools/${id}`, token);
    },
    /** GET /api/v1/admin/ip-pools/:id/allocations */
    listAllocations(token: string, id: string): Promise<unknown[]> {
        return get(`${V1}/admin/ip-pools/${id}/allocations`, token);
    },
    /** POST /api/v1/admin/ip-pools/:id/allocations  { allocated_to?, purpose?, ip_address? } */
    allocate(token: string, id: string, body?: { allocated_to?: string; purpose?: string; ip_address?: string }): Promise<unknown> {
        return post(`${V1}/admin/ip-pools/${id}/allocations`, token, body);
    },
    /** DELETE /api/v1/admin/ip-pools/:id/allocations/:ip */
    release(token: string, id: string, ip: string): Promise<void> {
        return del(`${V1}/admin/ip-pools/${id}/allocations/${ip}`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// ADMIN — PLATFORM CONFIG
// ═══════════════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════════════
// SERVICE ENDPOINTS (MQ / SMTP / Redis) — admin CRUD + tenant list
// ═══════════════════════════════════════════════════════════════════════════════

export const mqEndpoints = {
    /** GET /api/v1/mq-endpoints — tenant-safe (id+name only) */
    list(token: string): Promise<Array<{ id: string; name: string }>> {
        return get(`${V1}/mq-endpoints`, token);
    },
};
export const adminMqEndpoints = {
    list(token: string): Promise<any[]> { return get(`${V1}/admin/mq-endpoints`, token); },
    create(token: string, body: any): Promise<{ id: string }> { return post(`${V1}/admin/mq-endpoints`, token, body); },
    update(token: string, id: string, body: any): Promise<void> { return put(`${V1}/admin/mq-endpoints/${id}`, token, body); },
    delete(token: string, id: string): Promise<void> { return del(`${V1}/admin/mq-endpoints/${id}`, token); },
};

export const smtpEndpoints = {
    list(token: string): Promise<Array<{ id: string; name: string }>> {
        return get(`${V1}/smtp-endpoints`, token);
    },
};
export const adminSmtpEndpoints = {
    list(token: string): Promise<any[]> { return get(`${V1}/admin/smtp-endpoints`, token); },
    create(token: string, body: any): Promise<{ id: string }> { return post(`${V1}/admin/smtp-endpoints`, token, body); },
    update(token: string, id: string, body: any): Promise<void> { return put(`${V1}/admin/smtp-endpoints/${id}`, token, body); },
    delete(token: string, id: string): Promise<void> { return del(`${V1}/admin/smtp-endpoints/${id}`, token); },
};

export const redisEndpoints = {
    list(token: string): Promise<Array<{ id: string; name: string }>> {
        return get(`${V1}/redis-endpoints`, token);
    },
};
export const adminRedisEndpoints = {
    list(token: string): Promise<any[]> { return get(`${V1}/admin/redis-endpoints`, token); },
    create(token: string, body: any): Promise<{ id: string }> { return post(`${V1}/admin/redis-endpoints`, token, body); },
    update(token: string, id: string, body: any): Promise<void> { return put(`${V1}/admin/redis-endpoints/${id}`, token, body); },
    delete(token: string, id: string): Promise<void> { return del(`${V1}/admin/redis-endpoints/${id}`, token); },
};

export const adminPlatform = {
    /** GET /api/v1/admin/platform-config */
    list(token: string): Promise<Array<{ key: string; value: string; description?: string }>> {
        return get(`${V1}/admin/platform-config`, token);
    },
    /** POST /api/v1/admin/platform-config  { key, value } */
    set(token: string, key: string, value: string): Promise<unknown> {
        return post(`${V1}/admin/platform-config`, token, { key, value });
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// TEMPLATES (visible to any logged-in user; CRUD restricted to admin / project owner)
// ═══════════════════════════════════════════════════════════════════════════════

export interface TemplateDto {
    id: string;
    slug: string;
    name: string;
    icon_url: string | null;
    category: 'app' | 'database';
    description: string | null;
    visibility: 'PUBLIC' | 'ORG' | 'PRIVATE';
    owner_user_id: string | null;
    owner_project_id: string | null;
    // First-class image identity
    image_registry_id: string | null;
    image_repository: string;     // 'mysql', 'ghcr.io/owner/repo'
    image_tag: string;            // 'latest', '8.4'
    image_digest: string | null;  // optional sha256:...
    image_ref: string;            // server-rendered '[registry/]repo[:tag][@digest]'
    spec: any;
    requirements: any[];
    inputs: any[];
    is_active: boolean;
    version: number;
}

// A declared requirement is mandatory at deploy time. There is no `required`
// field — declaration alone means the user must bind it.
export interface TemplateRequirement {
    key: string;
    kind: 'database' | 'objstore' | 'cache' | 'mq' | 'smtp';
    engine?: string;
    label?: string;
    /** logical key (host/port/password/…) -> env var name to inject */
    env_mapping?: Record<string, string>;
    /** Per-requirement config files, rendered with minijinja from resolved attrs. */
    config_files?: Array<{ path: string; template: string }>;
    binding_modes?: Array<'managed' | 'provision'>;
}

export interface TemplateBindingChoice {
    requirement_key: string;
    mode: 'managed' | 'provision';
    managed_ref_id?: string;
    provision_cluster_id?: string;
    provision_name_hint?: string;
}

export interface TemplateDeployRequest {
    template_id: string;
    app_name: string;
    display_name?: string;
    bindings?: TemplateBindingChoice[];
    input_overrides?: Record<string, any>;
}

export interface TemplateUpsertBody {
    slug: string;
    name: string;
    icon_url?: string | null;
    category?: 'app' | 'database';
    description?: string | null;
    image_registry_id?: string | null;
    image_repository: string;
    image_tag?: string;
    image_digest?: string | null;
    spec: any;
    requirements?: any[];
    inputs?: any[];
    is_active?: boolean;
}

export const templates = {
    /** GET /api/v1/templates — all templates visible to the caller */
    list(token: string): Promise<TemplateDto[]> {
        return get(`${V1}/templates`, token);
    },
    /** GET /api/v1/templates/:id */
    get(token: string, id: string): Promise<TemplateDto> {
        return get(`${V1}/templates/${id}`, token);
    },
};

export const adminTemplates = {
    /** POST /api/v1/admin/templates */
    create(token: string, body: TemplateUpsertBody): Promise<{ id: string }> {
        return post(`${V1}/admin/templates`, token, body);
    },
    /** PUT /api/v1/admin/templates/:id */
    update(token: string, id: string, body: TemplateUpsertBody): Promise<void> {
        return put(`${V1}/admin/templates/${id}`, token, body);
    },
    /** DELETE /api/v1/admin/templates/:id */
    delete(token: string, id: string): Promise<void> {
        return del(`${V1}/admin/templates/${id}`, token);
    },
};

export const projectTemplates = {
    /** POST /api/v1/projects/:project_id/templates */
    create(token: string, projectId: string, body: TemplateUpsertBody): Promise<{ id: string }> {
        return post(`${V1}/projects/${projectId}/templates`, token, body);
    },
    /** PUT /api/v1/projects/:project_id/templates/:id */
    update(token: string, projectId: string, id: string, body: TemplateUpsertBody): Promise<void> {
        return put(`${V1}/projects/${projectId}/templates/${id}`, token, body);
    },
    /** DELETE /api/v1/projects/:project_id/templates/:id */
    delete(token: string, projectId: string, id: string): Promise<void> {
        return del(`${V1}/projects/${projectId}/templates/${id}`, token);
    },
};

// ═══════════════════════════════════════════════════════════════════════════════
// Default export — grouped namespace
// ═══════════════════════════════════════════════════════════════════════════════

const backendApi = {
    auth,
    profile,
    plans,
    subscription,
    billing,
    projects,
    apps,
    databases,
    s3Targets,
    network,
    backups,
    monitoring,
    adminUsers,
    adminProjects,
    adminBilling,
    adminLdap,
    adminPlans,
    adminNodes,
    adminClusters,
    adminPools,
    adminDbClusters,
    adminRegistries,
    adminS3Targets,
    adminProxyManagers,
    adminIpPools,
    adminPlatform,
    templates,
    adminTemplates,
    projectTemplates,
    mqEndpoints,
    adminMqEndpoints,
    smtpEndpoints,
    adminSmtpEndpoints,
    redisEndpoints,
    adminRedisEndpoints,
};

export default backendApi;

// Named export for use in server actions (import { backend } from '...')
export const backend = backendApi;

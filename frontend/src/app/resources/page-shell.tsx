import BreadcrumbSetter from "@/components/breadcrumbs-setter";
import PageTitle from "@/components/custom/page-title";
import { getAdminUserSession, getBackendToken } from "@/server/utils/action-wrapper.utils";
import { getT } from "@/i18n/server";
import { redirect } from "next/navigation";
import { BackendApiError } from "@/server/adapter/backend-api.adapter";

export async function getAdminToken() {
    await getAdminUserSession();
    return getBackendToken();
}

/**
 * Safe fallback for page-level data fetching. Use instead of `.catch(() => [])`.
 * Re-throws 401 errors as a redirect to /auth so expired sessions don't
 * silently show empty pages.
 */
export function catchOrEmpty<T>(fallback: T) {
    return (err: unknown): T => {
        if (err instanceof BackendApiError && err.status === 401) {
            redirect('/auth?expired=1');
        }
        return fallback;
    };
}

export async function ResourcePageShell({
    title,
    subtitle,
    current,
    children,
}: {
    title: string;
    subtitle: string;
    current: string;
    children: React.ReactNode;
}) {
    const { t } = await getT();
    return (
        <div className="flex-1 space-y-6 pt-6 pb-16">
            <PageTitle title={title} subtitle={subtitle} />
            <BreadcrumbSetter items={[
                { name: t("nav.resourceManagement"), url: "/resources/pools" },
                { name: current },
            ]} />
            {children}
        </div>
    );
}

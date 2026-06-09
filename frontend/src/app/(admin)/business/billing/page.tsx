import { backend } from "@/server/adapter/backend-api.adapter";
import AdminBillingTab from "@/app/(admin)/settings/server/admin-billing-tab";
import { BusinessPageShell, getBusinessAdminToken, catchOrEmpty } from "../page-shell";
import { getT } from "@/i18n/server";

export default async function BusinessBillingPage() {
    const token = await getBusinessAdminToken();
    const [allWallets, allInvoices, allTransactions] = await Promise.all([
        backend.adminBilling.listWallets(token).catch(catchOrEmpty({ data: [] })),
        backend.adminBilling.listInvoices(token, { per_page: 50 }).catch(catchOrEmpty({ data: [] })),
        backend.adminBilling.listTransactions(token, { per_page: 80 }).catch(catchOrEmpty({ data: [] })),
    ]);
    const wallets = ((allWallets as any)?.data ?? allWallets ?? []) as any[];
    const invoices = ((allInvoices as any)?.data ?? allInvoices ?? []) as any[];
    const transactions = ((allTransactions as any)?.data ?? allTransactions ?? []) as any[];
    const { t } = await getT();

    return (
        <BusinessPageShell
            title={t("page.business.billing.title")}
            subtitle={t("page.business.billing.subtitle")}
            current={t("page.business.billing.title")}>
            <AdminBillingTab
                initialWallets={wallets}
                initialInvoices={invoices}
                initialTransactions={transactions}
            />
        </BusinessPageShell>
    );
}

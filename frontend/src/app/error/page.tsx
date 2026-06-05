
import { getT } from "@/i18n/server";

export default async function ErrorPage() {
    const { t } = await getT();

    return (
        <div>
            {t('common.error')}
        </div>
    )
}

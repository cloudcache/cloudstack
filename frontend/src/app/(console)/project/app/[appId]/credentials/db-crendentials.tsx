import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { useEffect, useState } from "react";
import { DatabaseTemplateInfoModel } from "@/shared/model/database-template-info.model";
import { Actions } from "@/frontend/utils/nextjs-actions.utils";
import { getDatabaseCredentials } from "./actions";
import CopyInputField from "@/components/custom/copy-input-field";
import FullLoadingSpinner from "@/components/ui/full-loading-spinnter";
import { useT } from "@/i18n";

export default function DbCredentials({
    app
}: {
    app: AppExtendedModel;
}) {

    const t = useT();
    const [databaseCredentials, setDatabaseCredentials] = useState<DatabaseTemplateInfoModel | undefined>(undefined);


    const loadCredentials = async (appId: string) => {
        const response = await Actions.run(() => getDatabaseCredentials(appId));
        setDatabaseCredentials(response);
    }

    useEffect(() => {
        loadCredentials(app.id);
        return () => {
            setDatabaseCredentials(undefined);
        }
    }, [app]);

    return <>
        <Card>
            <CardHeader>
                <CardTitle>{t('app.dbCredentials.title')}</CardTitle>
                <CardDescription>{t('app.dbCredentials.description')}</CardDescription>
            </CardHeader>
            <CardContent>
                {!databaseCredentials ? <FullLoadingSpinner /> : <>
                    <div className="grid grid-cols-2 gap-4">
                        {!!databaseCredentials?.databaseName && <>   <CopyInputField
                            label={t('app.dbCredentials.databaseName')}
                            value={databaseCredentials?.databaseName || ''} />

                            <div></div>
                        </>}

                        {!!databaseCredentials?.username && <CopyInputField
                            label={t('common.username')}
                            value={databaseCredentials?.username || ''} />}

                        {!!databaseCredentials?.password && <CopyInputField
                            label={t('common.password')}
                            secret={true}
                            value={databaseCredentials?.password || ''} />}

                        <CopyInputField
                            label={t('app.dbCredentials.internalHostname')}
                            value={databaseCredentials?.hostname || ''} />

                        <CopyInputField
                            label={t('app.dbCredentials.internalPort')}
                            value={(databaseCredentials?.port + '')} />
                    </div>
                    <div className="grid grid-cols-1 gap-4 pt-4">
                        <CopyInputField
                            label={t('app.dbCredentials.internalConnectionUrl')}
                            secret={true}
                            value={databaseCredentials?.internalConnectionUrl || ''} />
                    </div>
                </>}
            </CardContent>
        </Card>
    </>;
}

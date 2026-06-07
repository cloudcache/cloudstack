'use client';

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { useEffect, useState } from "react";
import { Actions } from "@/frontend/utils/nextjs-actions.utils";
import { getDatabaseCredentials } from "./actions";
import CopyInputField from "@/components/custom/copy-input-field";
import FullLoadingSpinner from "@/components/ui/full-loading-spinnter";
import { useT } from "@/i18n";

interface DbConnInfo {
    app_type: string;
    host: string;
    port: number;
    database: string;
    username: string;
    password: string;
}

/** A CLI connection command for an external client, one per database dialect. */
function buildCommands(c: DbConnInfo): { label: string; value: string }[] {
    switch (c.app_type) {
        case 'POSTGRES':
            return [{ label: 'psql', value: `psql "postgresql://${c.username}:${c.password}@${c.host}:${c.port}/${c.database}"` }];
        case 'MYSQL':
        case 'MARIADB':
            return [{ label: 'mysql', value: `mysql -h ${c.host} -P ${c.port} -u ${c.username} -p'${c.password}' ${c.database}` }];
        case 'MONGODB':
            return [{ label: 'mongosh', value: `mongosh "mongodb://${c.username}:${c.password}@${c.host}:${c.port}/${c.database}"` }];
        case 'REDIS':
            return [{ label: 'redis-cli', value: `redis-cli -h ${c.host} -p ${c.port}${c.password ? ` -a '${c.password}'` : ''}` }];
        default:
            return [];
    }
}

export default function DbToolsCard({
    app
}: {
    app: AppExtendedModel;
}) {
    const t = useT();
    const [conn, setConn] = useState<DbConnInfo | undefined>(undefined);

    useEffect(() => {
        let active = true;
        Actions.run(() => getDatabaseCredentials(app.id)).then(res => {
            if (active) setConn(res as DbConnInfo);
        });
        return () => { active = false; };
    }, [app]);

    return (
        <Card>
            <CardHeader>
                <CardTitle>{t('app.dbTools.title')}</CardTitle>
                <CardDescription>{t('app.dbTools.description')}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
                {!conn ? <FullLoadingSpinner /> : (
                    buildCommands(conn).map(cmd => (
                        <CopyInputField key={cmd.label} label={cmd.label} secret={true} value={cmd.value} />
                    ))
                )}
            </CardContent>
        </Card>
    );
}

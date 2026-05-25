'use client';

import { Card, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { useBreadcrumbs } from "@/frontend/states/zustand.states";
import { useEffect } from "react";
import { BackendApp } from "@/server/adapter/backend-api.adapter";

export default function AppBreadcrumbs({ app, apps, tabName }: { app: BackendApp; apps: { id: string; name: string }[]; tabName?: string }) {
    const { setBreadcrumbs } = useBreadcrumbs();
    useEffect(() => setBreadcrumbs([
        { name: "Projects", url: "/" },
        { name: app.project_name, url: "/project/" + app.project_id },
        {
            name: app.name,
            dropdownItems: apps.map(a => ({
                name: a.name,
                url: `/project/app/${a.id}${tabName ? `?tabName=${tabName}` : ''}`,
                active: a.id === app.id,
            })),
        },
    ]), []);
    return <></>;
}

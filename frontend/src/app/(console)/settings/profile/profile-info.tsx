'use client';

import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { useState } from "react";
import { Toast } from "@/frontend/utils/toast.utils";
import { updateProfile } from "./actions";

export default function ProfileInfo({
    displayName,
    username,
    email,
}: {
    displayName: string;
    username: string;
    email: string;
}) {
    const [name, setName] = useState(displayName ?? '');
    const [saving, setSaving] = useState(false);

    const submit = async () => {
        const trimmed = name.trim();
        if (!trimmed) return;
        setSaving(true);
        try {
            await Toast.fromAction(() => updateProfile(trimmed));
        } finally {
            setSaving(false);
        }
    };

    return (
        <Card>
            <CardHeader>
                <CardTitle>Profile information</CardTitle>
                <CardDescription>Update your display name. Username and email are managed by an administrator.</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
                <div className="grid gap-2">
                    <Label>Username</Label>
                    <Input value={username} disabled />
                </div>
                <div className="grid gap-2">
                    <Label>Email</Label>
                    <Input value={email} disabled />
                </div>
                <div className="grid gap-2">
                    <Label>Display name</Label>
                    <Input value={name} maxLength={128} onChange={e => setName(e.target.value)} />
                </div>
            </CardContent>
            <CardFooter>
                <Button onClick={submit} disabled={saving || !name.trim() || name.trim() === (displayName ?? '')}>
                    {saving ? 'Saving…' : 'Save'}
                </Button>
            </CardFooter>
        </Card>
    );
}

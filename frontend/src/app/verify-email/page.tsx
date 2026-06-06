'use client'

// Public landing page hit from the verification email link.
// The token from the URL is POSTed once on mount; the result is shown inline.

import { useEffect, useState } from "react";
import { useSearchParams, useRouter } from "next/navigation";
import { backend } from "@/server/adapter/backend-api.adapter";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Loader2, CheckCircle2, XCircle, Mail } from "lucide-react";

type Status = 'verifying' | 'ok' | 'fail';

export default function VerifyEmailPage() {
    const search = useSearchParams();
    const router = useRouter();
    const token = search.get('token') ?? '';
    const [status, setStatus] = useState<Status>('verifying');
    const [error, setError] = useState<string>('');
    const [resendEmail, setResendEmail] = useState('');
    const [resendBusy, setResendBusy] = useState(false);
    const [resendMsg, setResendMsg] = useState('');

    useEffect(() => {
        if (!token) {
            setStatus('fail');
            setError('Missing verification token in URL.');
            return;
        }
        backend.auth.verifyEmail(token)
            .then(() => setStatus('ok'))
            .catch((e: any) => {
                setStatus('fail');
                setError(e?.message ?? 'Verification failed.');
            });
    }, [token]);

    const resend = async () => {
        if (!resendEmail.includes('@')) {
            setResendMsg('Enter a valid email.');
            return;
        }
        setResendBusy(true);
        setResendMsg('');
        try {
            await backend.auth.resendVerification(resendEmail);
            setResendMsg('If the email is registered and unverified, a new link has been sent.');
        } catch (e: any) {
            setResendMsg(e?.message ?? 'Failed to resend.');
        } finally {
            setResendBusy(false);
        }
    };

    return (
        <div className="min-h-screen flex items-center justify-center p-4 bg-muted/30">
            <Card className="w-full max-w-md">
                <CardHeader>
                    <CardTitle className="flex items-center gap-2">
                        <Mail className="h-5 w-5" />
                        Email verification
                    </CardTitle>
                </CardHeader>
                <CardContent className="space-y-4">
                    {status === 'verifying' && (
                        <div className="flex items-center gap-2 text-muted-foreground">
                            <Loader2 className="h-4 w-4 animate-spin" /> Verifying…
                        </div>
                    )}
                    {status === 'ok' && (
                        <>
                            <div className="flex items-center gap-2 text-green-600">
                                <CheckCircle2 className="h-5 w-5" /> Email verified.
                            </div>
                            <Button className="w-full" onClick={() => router.push('/auth')}>
                                Go to login
                            </Button>
                        </>
                    )}
                    {status === 'fail' && (
                        <>
                            <div className="flex items-center gap-2 text-destructive">
                                <XCircle className="h-5 w-5" /> {error || 'Could not verify this link.'}
                            </div>
                            <div className="border-t pt-3 space-y-2">
                                <p className="text-sm text-muted-foreground">
                                    Need a new link? Enter your email below:
                                </p>
                                <Input
                                    type="email"
                                    placeholder="you@example.com"
                                    value={resendEmail}
                                    onChange={e => setResendEmail(e.target.value)} />
                                <Button className="w-full" onClick={resend} disabled={resendBusy}>
                                    {resendBusy ? 'Sending…' : 'Resend verification email'}
                                </Button>
                                {resendMsg && <p className="text-xs text-muted-foreground">{resendMsg}</p>}
                            </div>
                        </>
                    )}
                </CardContent>
            </Card>
        </div>
    );
}

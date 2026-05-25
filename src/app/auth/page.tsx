'use server'

import { backend } from "@/server/adapter/backend-api.adapter";
import UserRegistrationForm from "./register-from";
import UserLoginForm from "./login-form";
import { getUserSession } from "@/server/utils/action-wrapper.utils";
import { redirect } from "next/navigation";

export default async function AuthPage() {
    const session = await getUserSession();
    if (session) {
        redirect('/');
    }
    let registrationOpen = false;
    try {
        const status = await backend.auth.registrationStatus();
        registrationOpen = status.enabled;
    } catch {
        // If backend unreachable, show login form
    }
    return (
        <div className="flex items-center justify-center" style={{ height: '95vh' }}>
            {registrationOpen ? <UserRegistrationForm /> : <UserLoginForm />}
        </div>
    )
}

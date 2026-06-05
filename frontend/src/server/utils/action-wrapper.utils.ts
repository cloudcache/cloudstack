import { ServiceException } from "@/shared/model/service.exception.model";
import { UserSession } from "@/shared/model/sim-session.model";
import { getServerSession } from "next-auth";
import { ZodRawShape, ZodObject, objectUtil, baseObjectOutputType, z, ZodType } from "zod";
import { redirect } from "next/navigation";
import { ServerActionResult } from "@/shared/model/server-action-error-return.model";
import { FormValidationException } from "@/shared/model/form-validation-exception.model";
import { authOptions } from "@/server/utils/auth-options";
import { NextResponse } from "next/server";
import { RolePermissionEnum } from "@/shared/model/role-extended.model.ts";
import { BackendApiError } from "@/server/adapter/backend-api.adapter";

// ─── Session helpers ──────────────────────────────────────────────────────────

function isBackendTokenExpired(token: string | undefined): boolean {
    if (!token) return true;              // missing or empty → treat as expired
    const [, payload] = token.split('.');
    if (!payload) return true;            // not a valid JWT → treat as expired

    try {
        const normalized = payload.replace(/-/g, '+').replace(/_/g, '/');
        const decoded = JSON.parse(Buffer.from(normalized, 'base64').toString('utf8'));
        return typeof decoded.exp === 'number' && decoded.exp * 1000 <= Date.now();
    } catch {
        return true;
    }
}

/**
 * Returns a UserSession if a user is logged in, or null.
 * Reads solely from the NextAuth JWT — no Prisma call.
 * isGlobalAdmin comes from the Rust backend JWT stored in the session.
 */
export async function getUserSession(): Promise<UserSession | null> {
    const session = await getServerSession(authOptions);
    if (!session?.user?.email) return null;
    if (isBackendTokenExpired(session.backendToken)) return null;

    return {
        email: session.user.email,
        // Represent admin status via a synthetic userGroup so legacy
        // UserGroupUtils.isAdmin() still works during the migration.
        userGroup: session.user.isGlobalAdmin
            ? {
                name: 'admin',
                id: 'admin',
                canAccessBackups: true,
                roleProjectPermissions: [],
            }
            : undefined,
    };
}

export async function getAuthUserSession(): Promise<UserSession> {
    const rawSession = await getServerSession(authOptions);
    if (isBackendTokenExpired(rawSession?.backendToken)) {
        redirect('/auth?expired=1');
    }

    const session = await getUserSession();
    if (!session) {
        console.error('User is not authenticated.');
        redirect('/auth');
    }
    return session;
}

/**
 * Returns the Rust backend JWT stored in the NextAuth session.
 * Use this in server actions / API routes that call the Rust backend.
 * Redirects to /auth if the user is not logged in.
 */
export async function getBackendToken(): Promise<string> {
    const session = await getServerSession(authOptions);
    if (!session?.backendToken) {
        redirect('/auth');
    }
    if (isBackendTokenExpired(session.backendToken)) {
        redirect('/auth?expired=1');
    }
    return session.backendToken;
}

export async function getAdminUserSession(): Promise<UserSession> {
    const session = await getServerSession(authOptions);
    if (!session?.user) {
        redirect('/auth');
    }
    if (isBackendTokenExpired(session.backendToken)) {
        redirect('/auth?expired=1');
    }
    if (!session.user.isGlobalAdmin) {
        console.error('User is not admin.');
        throw new ServiceException('User is not authorized for this action.');
    }
    return {
        email: session.user.email,
        userGroup: {
            name: 'admin',
            id: 'admin',
            canAccessBackups: true,
            roleProjectPermissions: [],
        },
    };
}

export async function isAuthorizedForBackups() {
    // Authorization delegated to the Rust backend (returns 403 if not allowed).
    // We only verify the user has an active session here.
    return getAuthUserSession();
}

export async function isAuthorizedReadForApp(appId: string) {
    // Authorization is enforced by the Rust backend when the action calls it.
    // We only verify the user is authenticated.
    return getAuthUserSession();
}

export async function isAuthorizedWriteForApp(appId: string) {
    return getAuthUserSession();
}

export async function safeGetUserPermissionForApp(appId: string): Promise<RolePermissionEnum | null> {
    const session = await getUserSession();
    if (!session) return null;
    // Global admin always has full access.
    if (session.userGroup?.name === 'admin') return RolePermissionEnum.READWRITE;
    // For non-admins: the backend enforces permission; return READWRITE as optimistic default.
    // Pages that need to show/hide write controls should call the backend to get role.
    return RolePermissionEnum.READ;
}

// ─── Action wrappers ──────────────────────────────────────────────────────────

export async function saveFormAction<ReturnType, TInputData, ZodType extends ZodRawShape>(
    inputData: TInputData,
    validationModel: ZodObject<ZodType>,
    func: (validateData: { [k in keyof objectUtil.addQuestionMarks<baseObjectOutputType<ZodType>, any>]: objectUtil.addQuestionMarks<baseObjectOutputType<ZodType>, any>[k]; }) => Promise<ReturnType>,
    redirectOnSuccessPath?: string,
    ignoredFields: (keyof ZodType)[] = []) {
    return simpleAction<ReturnType, z.infer<typeof validationModel>>(async () => {

        // Omit ignored fields from validation model
        const omitBody = {};
        const allIgnoreFiels = ['createdAt', 'updatedAt', ...ignoredFields];
        allIgnoreFiels.forEach(field => (omitBody as any)[field] = true);
        const schemaWithoutIgnoredFields = validationModel.omit(omitBody);

        const validatedFields = schemaWithoutIgnoredFields.safeParse(inputData);
        if (!validatedFields.success) {
            console.error('Validation failed for input:', inputData, 'with errors:', validatedFields.error.flatten().fieldErrors);
            throw new FormValidationException('Please correct the errors in the form.', validatedFields.error.flatten().fieldErrors);
        }

        if (!validatedFields.data) {
            console.error('No data available after validation of input:', validatedFields.data);
            throw new ServiceException('An unknown error occurred.');
        }
        return await func(validatedFields.data);
    }, redirectOnSuccessPath);
}

export async function simpleAction<ReturnType, ValidationCallbackType>(
    func: () => Promise<ReturnType>,
    redirectOnSuccessPath?: string) {
    let funcResult: ReturnType;
    try {
        funcResult = await func();
    } catch (ex) {
        if (ex instanceof FormValidationException) {
            return {
                status: 'error',
                message: ex.message,
                errors: ex.errors ?? undefined
            } as ServerActionResult<ValidationCallbackType, ReturnType>;
        } else if (ex instanceof ServiceException) {
            return {
                status: 'error',
                message: ex.message
            } as ServerActionResult<ValidationCallbackType, ReturnType>;
        } else if (ex instanceof BackendApiError) {
            return {
                status: 'error',
                message: ex.status === 401
                    ? 'Your session has expired. Please sign in again.'
                    : ex.message
            } as ServerActionResult<ValidationCallbackType, ReturnType>;
        } else {
            console.error(ex)
            return {
                status: 'error',
                message: 'An unknown error occurred.'
            } as ServerActionResult<ValidationCallbackType, ReturnType>;
        }
    }
    if (redirectOnSuccessPath) redirect(redirectOnSuccessPath);

    if (funcResult instanceof ServerActionResult) {
        return {
            status: funcResult.status,
            message: funcResult.message,
            errors: funcResult.errors,
            data: funcResult.data
        } as ServerActionResult<ValidationCallbackType, typeof funcResult.data>;
    }
    return {
        status: 'success',
        data: funcResult ?? undefined
    } as ServerActionResult<ValidationCallbackType, ReturnType>;
}

/**
 * Wrapper for server actions that handle file uploads via FormData
 * Extracts file from FormData and passes it to the handler function
 */
export async function fileUploadAction<ReturnType>(
    formData: FormData,
    fileFieldName: string,
    func: (file: File) => Promise<ReturnType>,
    redirectOnSuccessPath?: string) {
    let funcResult: ReturnType;
    try {
        const file = formData.get(fileFieldName) as File;
        if (!file || !file.size) {
            throw new ServiceException('No file uploaded or file is empty.');
        }
        funcResult = await func(file);
    } catch (ex) {
        if (ex instanceof ServiceException) {
            return {
                status: 'error',
                message: ex.message
            } as ServerActionResult<any, ReturnType>;
        } else {
            console.error(ex);
            return {
                status: 'error',
                message: 'An unknown error occurred during file upload.'
            } as ServerActionResult<any, ReturnType>;
        }
    }
    if (redirectOnSuccessPath) redirect(redirectOnSuccessPath);

    if (funcResult instanceof ServerActionResult) {
        return {
            status: funcResult.status,
            message: funcResult.message,
            errors: funcResult.errors,
            data: funcResult.data
        } as ServerActionResult<any, typeof funcResult.data>;
    }
    return {
        status: 'success',
        data: funcResult ?? undefined
    } as ServerActionResult<any, ReturnType>;
}

export async function simpleRoute<ReturnType>(
    func: () => Promise<ReturnType>) {
    let funcResult: ReturnType;
    try {
        funcResult = await func();
    } catch (ex) {
        if (ex instanceof FormValidationException) {
            return NextResponse.json({
                status: 'error',
                message: ex.message
            });
        } else if (ex instanceof ServiceException) {
            return NextResponse.json({
                status: 'error',
                message: ex.message
            });
        } else {
            console.error(ex)
            return NextResponse.json({
                status: 'error',
                message: 'An unknown error occurred.'
            });
        }
    }
    return funcResult;
}

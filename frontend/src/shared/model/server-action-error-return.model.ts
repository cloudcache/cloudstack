import { FormZodErrorValidationCallback } from "@/frontend/utils/form.utilts";

export class ServerActionResult<TErrorData, TReturnData> {

    constructor(public readonly status: 'error' | 'success',
        public readonly data?: TReturnData | void,
        public readonly message?: string,
        public readonly errors?: FormZodErrorValidationCallback<TErrorData>) {

    }
}

export class SuccessActionResult<T> extends ServerActionResult<undefined, T> {
    constructor(data?: T, message?: string) {
        super('success', data, message);
    }
}

export class ErrorActionResult<TErrorData> extends ServerActionResult<TErrorData, undefined> {
    constructor(errors?: FormZodErrorValidationCallback<TErrorData>, message?: string) {
        super('error', undefined, message, errors);
    }
}
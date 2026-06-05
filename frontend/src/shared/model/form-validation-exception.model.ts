import { FormZodErrorValidationCallback } from "@/frontend/utils/form.utilts";
import { ServiceException } from "./service.exception.model";
import { z, ZodType } from "zod";

export class FormValidationException<T extends ZodType<any, any, any>> extends ServiceException {
    constructor(message: string, public readonly errors: FormZodErrorValidationCallback<z.infer<T>> | null) {
        super(message);
        this.name = FormValidationException.name;
        // Optionally, you can capture the stack trace here if needed
        Error.captureStackTrace(this, this.constructor);
    }
}
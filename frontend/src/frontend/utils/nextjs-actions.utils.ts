import { ServerActionResult } from "@/shared/model/server-action-error-return.model";
import { toast } from "sonner";

const unknownErrorMessage = () => {
    if (typeof document !== 'undefined' && document.cookie.includes('qs_locale=zh')) {
        return '发生未知错误。';
    }
    return 'An unknown error occurred.';
};

export class Actions {
    static async run<TReturnData>(action: () => Promise<ServerActionResult<unknown, TReturnData>>) {
        try {
            const retVal = await action();
            if (!retVal || (retVal as ServerActionResult<unknown, TReturnData>).status !== 'success') {
                toast.error(retVal?.message ?? unknownErrorMessage());
                throw new Error(retVal?.message ?? unknownErrorMessage());
            }
            return retVal.data!;
        } catch (error) {
            toast.error(unknownErrorMessage());
            throw error;
        }
    }
}

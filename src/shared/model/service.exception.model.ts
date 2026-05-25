export class ServiceException extends Error {
    constructor(message: string) {
        super(message);
        this.name = ServiceException.name;
        // Optionally, you can capture the stack trace here if needed
        Error.captureStackTrace(this, this.constructor);
    }
}
import { isValidElement } from "react";

export class ReactNodeUtils {
    static getTextFromReactElement(element: React.ReactNode): string {
        if (typeof element === "string" || typeof element === "number") {
            return element.toString();
        }
        if (isValidElement(element)) {
            return this.getTextFromReactElement(element.props.children);
        }
        if (Array.isArray(element)) {
            return element.map(child => this.getTextFromReactElement(child)).join("");
        }
        return "";
    }
}
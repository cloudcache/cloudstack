import { TerminalSetupInfoModel } from "../model/terminal-setup-info.model";

export class StreamUtils {

    static getInputStreamName(terminalInfo: TerminalSetupInfoModel) {
        return `${terminalInfo.terminalSessionKey}_input`;
    }

    static getOutputStreamName(terminalInfo: TerminalSetupInfoModel) {
        return `${terminalInfo.terminalSessionKey}_output`;
    }
}
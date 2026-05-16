import { type IDataObject, type IExecuteFunctions } from 'n8n-workflow';
export type ValidationTarget = 'code' | 'fact' | 'action';
export type EnforcementMode = 'inherit' | 'advisory' | 'critical_fail_closed' | 'full_fail_closed';
export interface TieValidationRequest extends IDataObject {
    request_id?: string;
    target_type: ValidationTarget;
    enforcement_mode?: Exclude<EnforcementMode, 'inherit'>;
    subject: {
        content?: string;
        language?: string;
        claim?: string;
        action_type?: string;
        payload?: IDataObject;
    };
    evidence?: IDataObject[];
    context?: IDataObject;
    metadata?: IDataObject;
    webhook?: {
        callback_url: string;
        event_types?: string[];
    };
}
export declare function parseJsonInput(executeFunctions: IExecuteFunctions, itemIndex: number, fieldName: string): IDataObject | undefined;
export declare function parseJsonArrayInput(executeFunctions: IExecuteFunctions, itemIndex: number, fieldName: string): IDataObject[] | undefined;
export declare function normalizeBaseUrl(baseUrl: string, executeFunctions: IExecuteFunctions, itemIndex: number): string;
export declare function buildValidationRequest(executeFunctions: IExecuteFunctions, itemIndex: number): TieValidationRequest;

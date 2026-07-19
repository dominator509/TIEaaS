import type { IDataObject, IExecuteFunctions } from 'n8n-workflow';
export interface TieApiOptions {
    method?: 'GET' | 'POST';
    path: string;
    body?: IDataObject;
    query?: IDataObject;
}
export declare function tieApiRequest(executeFunctions: IExecuteFunctions, itemIndex: number, options: TieApiOptions): Promise<IDataObject>;

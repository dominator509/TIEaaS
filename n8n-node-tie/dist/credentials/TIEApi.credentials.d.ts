import type { ICredentialDataDecryptedObject, ICredentialTestRequest, ICredentialType, INodeProperties } from 'n8n-workflow';
export declare class TIEApi implements ICredentialType {
    name: string;
    displayName: string;
    documentationUrl: string;
    properties: INodeProperties[];
    test: ICredentialTestRequest;
    authenticate: {
        type: "generic";
        properties: {
            headers: {
                'X-API-Key': string;
            };
        };
    };
    preAuthentication?(credentials: ICredentialDataDecryptedObject): Promise<ICredentialDataDecryptedObject>;
}

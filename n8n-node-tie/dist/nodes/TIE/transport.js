"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.tieApiRequest = tieApiRequest;
const n8n_workflow_1 = require("n8n-workflow");
const validation_1 = require("./validation");
async function tieApiRequest(executeFunctions, itemIndex, options) {
    const credentials = await executeFunctions.getCredentials('tieApi');
    const baseUrl = (0, validation_1.normalizeBaseUrl)(String(credentials.baseUrl), executeFunctions, itemIndex);
    const timeoutMs = Number(credentials.timeoutMs ?? 5000);
    try {
        const response = await executeFunctions.helpers.httpRequest({
            baseURL: baseUrl,
            url: options.path,
            method: options.method ?? 'POST',
            body: options.body,
            qs: options.query,
            json: true,
            timeout: timeoutMs,
            headers: {
                accept: 'application/json',
            },
        });
        if (response === null || typeof response !== 'object' || Array.isArray(response)) {
            return {
                data: response,
            };
        }
        return response;
    }
    catch (error) {
        throw new n8n_workflow_1.NodeApiError(executeFunctions.getNode(), error, {
            itemIndex,
            message: 'Request to the TIE service failed.',
            description: 'Check the TIE base URL, API key, timeout, and service availability.',
        });
    }
}

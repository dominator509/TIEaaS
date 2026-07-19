"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.parseJsonInput = parseJsonInput;
exports.parseJsonArrayInput = parseJsonArrayInput;
exports.normalizeBaseUrl = normalizeBaseUrl;
exports.buildValidationRequest = buildValidationRequest;
const n8n_workflow_1 = require("n8n-workflow");
const urlPattern = /^https?:\/\//i;
function parseJsonInput(executeFunctions, itemIndex, fieldName) {
    const rawValue = executeFunctions.getNodeParameter(fieldName, itemIndex, '');
    if (!rawValue) {
        return undefined;
    }
    try {
        const parsed = JSON.parse(rawValue);
        if (parsed === null || Array.isArray(parsed) || typeof parsed !== 'object') {
            throw new Error('Expected a JSON object');
        }
        return parsed;
    }
    catch (error) {
        throw new n8n_workflow_1.NodeOperationError(executeFunctions.getNode(), `Field "${fieldName}" must contain a valid JSON object. ${error.message}`, { itemIndex });
    }
}
function parseJsonArrayInput(executeFunctions, itemIndex, fieldName) {
    const rawValue = executeFunctions.getNodeParameter(fieldName, itemIndex, '');
    if (!rawValue) {
        return undefined;
    }
    try {
        const parsed = JSON.parse(rawValue);
        if (!Array.isArray(parsed)) {
            throw new Error('Expected a JSON array');
        }
        return parsed.map((entry, index) => {
            if (entry === null || Array.isArray(entry) || typeof entry !== 'object') {
                throw new Error(`Entry at index ${index} is not a JSON object`);
            }
            return entry;
        });
    }
    catch (error) {
        throw new n8n_workflow_1.NodeOperationError(executeFunctions.getNode(), `Field "${fieldName}" must contain a valid JSON array of objects. ${error.message}`, { itemIndex });
    }
}
function normalizeBaseUrl(baseUrl, executeFunctions, itemIndex) {
    const normalized = baseUrl.trim().replace(/\/+$/, '');
    if (!normalized || !urlPattern.test(normalized)) {
        throw new n8n_workflow_1.NodeOperationError(executeFunctions.getNode(), 'Credential field "Base URL" must be a valid http or https URL.', { itemIndex });
    }
    return normalized;
}
function buildValidationRequest(executeFunctions, itemIndex) {
    const targetType = executeFunctions.getNodeParameter('targetType', itemIndex);
    const enforcementMode = executeFunctions.getNodeParameter('enforcementMode', itemIndex, 'inherit');
    const webhookCallbackUrl = executeFunctions.getNodeParameter('webhookCallbackUrl', itemIndex, '');
    const webhookEventTypes = executeFunctions.getNodeParameter('webhookEventTypes', itemIndex, '');
    const language = executeFunctions.getNodeParameter('language', itemIndex, '');
    const codeContent = executeFunctions.getNodeParameter('codeContent', itemIndex, '');
    const factClaim = executeFunctions.getNodeParameter('factClaim', itemIndex, '');
    const actionType = executeFunctions.getNodeParameter('actionType', itemIndex, '');
    const actionPayload = parseJsonInput(executeFunctions, itemIndex, 'actionPayload');
    const context = parseJsonInput(executeFunctions, itemIndex, 'contextJson');
    const metadata = parseJsonInput(executeFunctions, itemIndex, 'metadataJson');
    const evidence = parseJsonArrayInput(executeFunctions, itemIndex, 'evidenceJson');
    const subject = {};
    if (targetType === 'code') {
        if (!codeContent.trim()) {
            throw new n8n_workflow_1.NodeOperationError(executeFunctions.getNode(), 'Code content is required for code validation.', {
                itemIndex,
            });
        }
        subject.content = codeContent;
        if (language.trim()) {
            subject.language = language.trim();
        }
    }
    if (targetType === 'fact') {
        if (!factClaim.trim()) {
            throw new n8n_workflow_1.NodeOperationError(executeFunctions.getNode(), 'Fact claim is required for fact validation.', {
                itemIndex,
            });
        }
        subject.claim = factClaim;
    }
    if (targetType === 'action') {
        if (!actionType.trim()) {
            throw new n8n_workflow_1.NodeOperationError(executeFunctions.getNode(), 'Action type is required for action validation.', {
                itemIndex,
            });
        }
        if (!actionPayload) {
            throw new n8n_workflow_1.NodeOperationError(executeFunctions.getNode(), 'Action payload is required for action validation.', {
                itemIndex,
            });
        }
        subject.action_type = actionType.trim();
        subject.payload = actionPayload;
    }
    const request = {
        target_type: targetType,
        subject,
    };
    if (enforcementMode !== 'inherit') {
        request.enforcement_mode = enforcementMode;
    }
    if (context) {
        request.context = context;
    }
    if (metadata) {
        request.metadata = metadata;
    }
    if (evidence && evidence.length > 0) {
        request.evidence = evidence;
    }
    if (webhookCallbackUrl.trim()) {
        if (!urlPattern.test(webhookCallbackUrl.trim())) {
            throw new n8n_workflow_1.NodeOperationError(executeFunctions.getNode(), 'Webhook callback URL must be a valid http or https URL.', { itemIndex });
        }
        request.webhook = {
            callback_url: webhookCallbackUrl.trim(),
        };
        const events = webhookEventTypes
            .split(',')
            .map((value) => value.trim())
            .filter(Boolean);
        if (events.length > 0) {
            request.webhook.event_types = events;
        }
    }
    return request;
}

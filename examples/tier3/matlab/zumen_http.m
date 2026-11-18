function zumen_http(base, drawing)
% 起動済みの zumen serve --http で図面を開き、形式と図形の数を書く。
    [file, message] = fopen(drawing, 'rb');
    if file == -1
        error('zumen:input', '%s', message);
    end
    closeFile = onCleanup(@() fclose(file));
    data = fread(file, Inf, '*uint8');
    if ~feof(file)
        error('zumen:input', '%s', ferror(file));
    end
    clear closeFile

    [~, stem, extension] = fileparts(drawing);
    name = strcat(stem, extension);
    started = rpc(base, 'session_new', struct());
    session = started.session;
    closeSession = onCleanup(@() rpc(base, 'session_close', struct('session', session)));

    rpc(base, 'push', struct('session', session, 'name', name, ...
        'base64', matlab.net.base64encode(data)));
    opened = call(base, session, struct('v', 1, 'op', 'open', 'input', name));
    entities = call(base, session, struct('v', 1, 'op', 'entities', 'doc', name));
    fprintf('format %s\nentities %d\n', opened.result.format, entities.result.total);
end

function response = call(base, session, request)
    reply = rpc(base, 'call', struct('session', session, 'request', request));
    response = jsondecode(reply.response);
end

function result = rpc(base, method, params)
    options = weboptions('MediaType', 'application/json', 'ContentType', 'json');
    reply = webwrite(base, struct('jsonrpc', '2.0', 'id', 1, ...
        'method', method, 'params', params), options);
    if isfield(reply, 'error')
        error('zumen:rpc', '%s: %s', method, reply.error.message);
    end
    result = reply.result;
    if isfield(result, 'status') && ~strcmp(result.status, 'ok')
        error('zumen:status', '%s: %s', method, result.status);
    end
end

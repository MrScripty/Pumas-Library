from types import SimpleNamespace
async def health(): return {'status':'ok','protocol':3}
def create_app(): return SimpleNamespace(routes=[SimpleNamespace(path='/health',endpoint=health)])

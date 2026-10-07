from types import SimpleNamespace
__version__='2.14.0+cpu'
version=SimpleNamespace(cuda=None,hip=None)
cuda=SimpleNamespace(is_available=lambda:False)
backends=SimpleNamespace(mps=SimpleNamespace(is_available=lambda:False))
class Tensor:
 def __init__(self,values): self.values=values
 def __matmul__(self,other):
  return Tensor([[sum(a*b for a,b in zip(row,column)) for column in zip(*other.values)] for row in self.values])
 def tolist(self): return self.values
 def cpu(self): return self
def ones(shape): return Tensor([[1.0 for _ in range(shape[1])] for _ in range(shape[0])])

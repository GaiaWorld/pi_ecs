# 2022.2.25

## 脏优化问题

Changed脏列表优化，监听器在将实体记录到脏列表之前，应该判断该查询的其他过滤器是否存在Width、WithOut过滤器，在满足这类过滤器前提条件下的实体才能记录在脏列表。

### 处理进度
暂不处理，因为，在后续，这个判断可能失效，应该如何处理？
**比如**：Query<Node, &Matrix, (Changed<Matrix>, With<Text>)>, 在Matrix改变时，可能还不存在Text组件，如果此时不记录脏，并且对应实体添加了一个Text组价，该脏编辑始终无法进入脏列表。

应该如何做？TODO
void bubble_sort(int *array, int size);
int main()
{
    int arr[4];
    arr[0] = 1;
    arr[1] = 3;
    arr[2] = 4;
    arr[3] = 2;
    int size = 4;
    bubble_sort(arr, size);
    int res = 0;
    for (int i = 0; i < size; i++)
    {
        res += i * arr[i];
    }
    return res;
}

void bubble_sort(int *array, int size)
{
    int replace = 1;
    while (replace != 0)
    {
        replace = 0;
        for (int i = 1; i < size; i++)
        {
            if (array[i - 1] > array[i])
            {
                int tmp = array[i];
                array[i] = array[i - 1];
                array[i - 1] = tmp;
                replace++;
            }
        }
    }
}
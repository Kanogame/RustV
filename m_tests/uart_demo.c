
void print(char *string);
int main()
{
    volatile char *text = "hi there there there there there there there there there there there there there there there there there there there v";
    volatile char *text2 = "hi there there there there there asfasdfthere there there there there there v";
    print((char *)text2);
    print((char *)text);
    return 0;
}

void print(char *string)
{
    volatile char *uart = (volatile char *)0x10000000;
    int i = 0;
    while (string[i] != '\0')
    {
        uart[0] = string[i];
        i++;
    }
}